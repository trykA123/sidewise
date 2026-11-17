//! The app's one cache of open Git sides, shared by the folder session and the diff builder.
//! A screen of the compare tab; the opener runs on the session thread, never the UI thread.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};

use sidewise_core::{Location, ProviderPath};
use sidewise_engine::model::{EntryKind, Reason, RelPath, SideEntry, SideSource};
use sidewise_git::location::{Form, GitAt, parse};
use sidewise_git::repo::Commit;
use sidewise_git::side::GitTreeSide;
use sidewise_host::compare::diff::hooks::SideFn;
use sidewise_host::compare::diff::sides::side_of;
use sidewise_host::compare::handle::CompareTarget;
use sidewise_host::compare::pane::LocalTarget;

/// The app's one cache, shared by the compare engine and every diff backend; made on first use.
pub fn shared_sides() -> Arc<GitSides> {
    static SIDES: OnceLock<Arc<GitSides>> = OnceLock::new();
    Arc::clone(SIDES.get_or_init(Arc::default))
}

/// What names a side: its form, repository folder and revision text.
type Key = (Form, PathBuf, String);

/// How the cache opens a side: `GitTreeSide::open`, or `open_isolated` in tests.
type Opener = fn(&Path, &str, Form) -> Result<GitTreeSide, Reason>;

/// A cache of the Git sides open tabs use, most recent last. It holds weak references: the
/// compare sessions own the sides, so a closed tab releases its repository files.
pub struct GitSides {
    open: Opener,
    entries: Mutex<Vec<(Key, Weak<GitTreeSide>)>>,
}

impl Default for GitSides {
    fn default() -> Self {
        GitSides {
            open: GitTreeSide::open,
            entries: Mutex::default(),
        }
    }
}

impl GitSides {
    /// A cache that opens with only the repository's own config, for tests.
    #[cfg(test)]
    pub fn isolated() -> Self {
        GitSides {
            open: GitTreeSide::open_isolated,
            entries: Mutex::default(),
        }
    }

    /// The newest live side for `at`, opened on a miss. Fails as `GitTreeSide::open`.
    /// Blocking on a miss: call off the UI thread. The open runs outside the lock.
    pub fn side(&self, at: &GitAt) -> Result<Arc<GitTreeSide>, Reason> {
        let key = key_of(at);
        match self.live(&key) {
            Some(hit) => Ok(hit),
            None => self.open_and_keep(at, key),
        }
    }

    /// As `side`, for a new compare run: the cached side only while its revision still names
    /// the same commit, else a new side that becomes the newest entry. The old side is never
    /// changed (another tab may read it). A revision that no longer resolves fails as `open`.
    pub fn fresh(&self, at: &GitAt) -> Result<Arc<GitTreeSide>, Reason> {
        let key = key_of(at);
        if let Some(hit) = self.live(&key)
            && hit.resolve_now()? == hit.commit()
        {
            return Ok(hit);
        }
        self.open_and_keep(at, key)
    }

    /// The live side held under `key`, moved to the most recent place.
    fn live(&self, key: &Key) -> Option<Arc<GitTreeSide>> {
        let mut list = lock(&self.entries);
        let found = list.iter().position(|(held, _)| held == key)?;
        let entry = list.remove(found);
        let side = entry.1.upgrade()?;
        list.push(entry);
        Some(side)
    }

    /// Opens `at` and keeps it as the newest side for `key`. Fails as the opener.
    fn open_and_keep(&self, at: &GitAt, key: Key) -> Result<Arc<GitTreeSide>, Reason> {
        let opened = Arc::new((self.open)(&at.repo, &at.rev, at.form)?);
        // Read outside the lock; `keep` uses it only when two threads opened different commits.
        let tip = opened.resolve_now().ok();
        let (side, displaced) = self.keep(key, opened, tip);
        // The last reference to a displaced side may close its repository: not under the lock.
        drop(displaced);
        Ok(side)
    }

    /// Stores `opened` under `key`. When a live side is already there, the one whose commit is
    /// `tip` (what the revision names now) wins; with no match, `opened` does. Prunes dead
    /// entries. Returns the side to use and the one replaced.
    fn keep(
        &self,
        key: Key,
        opened: Arc<GitTreeSide>,
        tip: Option<Commit>,
    ) -> (Arc<GitTreeSide>, Option<Arc<GitTreeSide>>) {
        let mut list = lock(&self.entries);
        list.retain(|(_, weak)| weak.strong_count() > 0);
        let held = list
            .iter()
            .position(|(found, _)| *found == key)
            .and_then(|found| list.remove(found).1.upgrade());
        let keep_held = held.as_ref().filter(|side| {
            let commit = side.commit();
            commit == opened.commit() || tip.as_ref() == Some(&commit)
        });
        if let Some(same) = keep_held {
            list.push((key, Arc::downgrade(same)));
            return (Arc::clone(same), Some(opened));
        }
        list.push((key, Arc::downgrade(&opened)));
        (opened, held)
    }
}

/// The cache key of a parsed Git location.
fn key_of(at: &GitAt) -> Key {
    (at.form, at.repo.clone(), at.rev.clone())
}

/// Locks the cache, taking the list back from a poisoned lock: it is only ever whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Accepts only a file where the diff needs one: a folder is `Unsupported`, a link `Link`
/// and a submodule `Submodule`.
fn file_only(kind: EntryKind) -> Result<(), Reason> {
    match kind {
        EntryKind::File => Ok(()),
        EntryKind::Folder => Err(Reason::Unsupported),
        EntryKind::Link => Err(Reason::Link),
        EntryKind::Submodule => Err(Reason::Submodule),
    }
}

/// A target builder: local locations open as local folders, Git locations through
/// `GitSides::fresh`, so each run sees a moved branch (unparsable: `Reason::Unsupported`).
pub fn target_for(sides: Arc<GitSides>) -> LocalTarget {
    Arc::new(move |location: &Location| {
        if location.is_local() {
            return CompareTarget::Local(location.clone());
        }
        let parsed = parse(location);
        let sides = Arc::clone(&sides);
        CompareTarget::Side(Box::new(move || {
            let at = parsed.map_err(|_| Reason::Unsupported)?;
            let side: Arc<dyn SideSource> = sides.fresh(&at)?;
            Ok(side)
        }))
    })
}

/// A diff side builder: local files as `side_of`, Git files from the newest side in `sides`.
/// A missing path or revision is `NotFound`, a non-file as `file_only`, any other provider
/// `Unsupported`.
pub fn side_fn(sides: Arc<GitSides>) -> SideFn {
    Arc::new(move |location, settings| {
        if location.is_local() {
            return side_of(location, settings);
        }
        let at = parse(location).map_err(|_| Reason::Unsupported)?;
        let side = sides.side(&at)?;
        let rel = RelPath::new(ProviderPath::from_text(&at.inner));
        file_only(side.kind(&rel)?)?;
        let stamp = side.stamp(&rel)?;
        let entry = SideEntry::new(rel, EntryKind::File, stamp.size, None);
        let source: Arc<dyn SideSource> = side;
        Ok((source, entry))
    })
}

#[cfg(test)]
#[path = "git_sides_tests.rs"]
mod tests;
