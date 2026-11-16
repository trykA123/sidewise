//! The model behind one compare tab: its two roots, the folder walk and the updates it receives.
//! It holds no GPUI handle; the tab owns it and drains the updates on the UI thread.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use navo_core::{Location, ProviderPath};
use navo_ui::browser::{CompareHooks, CompareSink, CompareUpdate, FolderSession, Waker};
use navo_ui::compare_diff::CompareOptions;

use crate::start::Side;
use crate::start_tabs::StartChoice;

/// What the two roots are: folders walked by the engine, or two files opened as one pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunKind {
    /// Two folders; the engine walks them into rows.
    Folders,
    /// Two files; no walk starts and the pair opens at once.
    Files,
}

/// Why a compare did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    /// The side named does not exist or cannot be read.
    Missing(Side),
    /// One side is a folder and the other is a file.
    Mixed,
    /// A side cannot be turned into a local location; the text says why.
    Location(String),
}

impl std::fmt::Display for RunError {
    /// Writes the user-facing English text of the error.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(Side::Left) => formatter.write_str("the left path does not exist"),
            Self::Missing(Side::Right) => formatter.write_str("the right path does not exist"),
            Self::Mixed => formatter.write_str("one path is a folder and the other is a file"),
            Self::Location(text) => write!(formatter, "the path cannot be opened: {text}"),
        }
    }
}

/// Classifies two roots by the file system: both folders, both files, or neither of those.
/// Fails with the side that cannot be read, or `Mixed` when the kinds differ.
pub fn classify(left: &Path, right: &Path) -> Result<RunKind, RunError> {
    let left = std::fs::metadata(left).map_err(|_| RunError::Missing(Side::Left))?;
    let right = std::fs::metadata(right).map_err(|_| RunError::Missing(Side::Right))?;
    match (left.is_dir(), right.is_dir()) {
        (true, true) => Ok(RunKind::Folders),
        (false, false) => Ok(RunKind::Files),
        _ => Err(RunError::Mixed),
    }
}

/// Turns a path into the local location the compare engine reads; keeps non-UTF-8 names on Unix.
#[cfg(unix)]
fn location_of(path: &Path) -> Result<Location, RunError> {
    use std::os::unix::ffi::OsStrExt as _;
    Location::local(ProviderPath::from_bytes(path.as_os_str().as_bytes()))
        .map_err(|error| RunError::Location(error.to_string()))
}

/// Turns a path into the local location the compare engine reads; keeps wide names on Windows.
#[cfg(windows)]
fn location_of(path: &Path) -> Result<Location, RunError> {
    use std::os::windows::ffi::OsStrExt as _;
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    Location::local(ProviderPath::from_wide(&wide))
        .map_err(|error| RunError::Location(error.to_string()))
}

/// One compare tab's run: the roots, the optional folder session and its update channel.
pub struct CompareRun {
    kind: RunKind,
    session: Option<FolderSession>,
    updates: Receiver<CompareUpdate>,
    roots: [Location; 2],
}

impl CompareRun {
    /// Classifies the roots and starts the walk for two folders; two files start no walk.
    /// `wake` is called after each update is queued; a bad root returns `RunError`.
    #[expect(
        clippy::too_many_arguments,
        reason = "the H1 brief fixes this signature"
    )]
    pub fn start(
        left: &Path,
        right: &Path,
        choice: &StartChoice,
        hooks: &dyn CompareHooks,
        wake: Waker,
    ) -> Result<Self, RunError> {
        let kind = classify(left, right)?;
        let roots = [location_of(left)?, location_of(right)?];
        let (sender, updates) = mpsc::channel();
        let session = match kind {
            RunKind::Folders => {
                let mut options = CompareOptions::default();
                options.compare.respect_gitignore = choice.respect_gitignore;
                let sink: CompareSink = Arc::new(move |update| {
                    let _ = sender.send(update);
                    wake();
                });
                let control = hooks.start((&roots[0], &roots[1]), &options, sink);
                Some(FolderSession::new(1, control, roots.clone()))
            }
            RunKind::Files => None,
        };
        Ok(Self {
            kind,
            session,
            updates,
            roots,
        })
    }

    /// Applies every queued update to the folder session; true when one of them changed it.
    pub fn drain(&mut self) -> bool {
        let mut applied = false;
        for update in self.updates.try_iter() {
            if let Some(session) = self.session.as_mut() {
                applied |= session.apply(update);
            }
        }
        applied
    }

    /// What the roots are: folders walked by the engine, or two files.
    pub fn kind(&self) -> RunKind {
        self.kind
    }

    /// The folder session of a folder run; `None` for two files.
    pub fn session(&self) -> Option<&FolderSession> {
        self.session.as_ref()
    }

    /// The folder session, mutable for cursor and view changes.
    pub fn session_mut(&mut self) -> Option<&mut FolderSession> {
        self.session.as_mut()
    }

    /// The two roots as local locations, left first.
    pub fn roots(&self) -> &[Location; 2] {
        &self.roots
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    use navo::compare::PaneCompare;

    use super::*;

    const WAIT: Duration = Duration::from_secs(20);

    /// A unique folder under the system temp root; its files go away when the guard drops.
    struct TempDir(PathBuf);

    impl TempDir {
        /// Creates the folder with `tag`, the process id and a counter in its name.
        fn new(tag: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let name = format!(
                "sidewise-h1-{tag}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            );
            let path = std::env::temp_dir().join(name);
            fs::create_dir(&path).expect("create temp dir");
            Self(path)
        }

        /// The folder's path.
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        /// Removes the folder and everything in it.
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A `Waker` that counts its calls, with the counter kept for the assertion.
    fn counting_wake() -> (Waker, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        let wakes = Arc::clone(&count);
        let wake: Waker = Arc::new(move || {
            wakes.fetch_add(1, Ordering::SeqCst);
        });
        (wake, count)
    }

    /// Two folders, two files, missing sides and a mixed pair each classify as the brief says.
    #[test]
    fn classify_reports_folders_files_and_bad_pairs() {
        let left = TempDir::new("classify-left");
        let right = TempDir::new("classify-right");
        assert_eq!(classify(left.path(), right.path()), Ok(RunKind::Folders));

        let one = left.path().join("one.txt");
        let two = right.path().join("two.txt");
        fs::write(&one, b"one").expect("write left file");
        fs::write(&two, b"two").expect("write right file");
        assert_eq!(classify(&one, &two), Ok(RunKind::Files));

        let missing = right.path().join("gone");
        assert_eq!(
            classify(&missing, right.path()),
            Err(RunError::Missing(Side::Left))
        );
        assert_eq!(
            classify(left.path(), &missing),
            Err(RunError::Missing(Side::Right))
        );
        assert_eq!(classify(left.path(), &one), Err(RunError::Mixed));
    }

    /// Every `RunError` writes the English text the tab's toast shows.
    #[test]
    fn run_error_display_is_english() {
        assert_eq!(
            RunError::Missing(Side::Left).to_string(),
            "the left path does not exist"
        );
        assert_eq!(
            RunError::Missing(Side::Right).to_string(),
            "the right path does not exist"
        );
        assert_eq!(
            RunError::Mixed.to_string(),
            "one path is a folder and the other is a file"
        );
        assert_eq!(
            RunError::Location("bad".to_owned()).to_string(),
            "the path cannot be opened: bad"
        );
    }

    /// A folder start walks both trees, wakes after updates and shows the root rows.
    #[test]
    fn folders_start_a_walk_and_drain_reports_the_rows() {
        let left = TempDir::new("walk-left");
        let right = TempDir::new("walk-right");
        for root in [left.path(), right.path()] {
            fs::write(root.join("same.txt"), b"one").expect("write same file");
            fs::create_dir(root.join("sub")).expect("create sub folder");
        }
        let (wake, wakes) = counting_wake();
        let mut run = CompareRun::start(
            left.path(),
            right.path(),
            &StartChoice::default(),
            &PaneCompare::default(),
            wake,
        )
        .expect("start folders");

        let deadline = Instant::now() + WAIT;
        let mut changed = false;
        loop {
            changed |= run.drain();
            if run.session().is_some_and(|session| !session.checking()) {
                break;
            }
            assert!(Instant::now() < deadline, "the walk did not finish");
            thread::sleep(Duration::from_millis(10));
        }

        assert!(run.session_mut().is_some(), "folder session");
        let session = run.session().expect("folder session");
        let mut rows: Vec<String> = session
            .visible()
            .iter()
            .map(|row| row.path.clone())
            .collect();
        rows.sort();
        assert_eq!(run.kind(), RunKind::Folders);
        assert_eq!(rows, ["same.txt", "sub"]);
        assert!(changed, "drain reported an applied update");
        assert!(wakes.load(Ordering::SeqCst) > 0, "the sink woke the tab");
    }

    /// Two files classify as a file run: no session starts and no wake fires.
    #[test]
    fn files_start_no_walk() {
        let left = TempDir::new("files-left");
        let right = TempDir::new("files-right");
        let left_file = left.path().join("a.txt");
        let right_file = right.path().join("b.txt");
        fs::write(&left_file, b"a").expect("write left file");
        fs::write(&right_file, b"b").expect("write right file");

        let (wake, wakes) = counting_wake();
        let mut run = CompareRun::start(
            &left_file,
            &right_file,
            &StartChoice::default(),
            &PaneCompare::default(),
            wake,
        )
        .expect("start files");

        assert_eq!(run.kind(), RunKind::Files);
        assert!(run.session().is_none());
        assert!(!run.drain());
        assert_eq!(wakes.load(Ordering::SeqCst), 0);
        assert_eq!(
            run.roots()[0],
            location_of(&left_file).expect("left location")
        );
    }
}
