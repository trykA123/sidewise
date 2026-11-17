//! The Git side the compare engine reads: one commit's tree, listed from an in-memory index
//! and read from the object database. Read-only, case-sensitive; never starts a program.

use std::io::{Cursor, Read};
use std::ops::ControlFlow;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use gix::error::Class;
use sidewise_core::{FileIdentity, ProviderId, ProviderPath};
use sidewise_engine::model::{
    CancelToken, EntryKind, ObjectId, ReadHint, Reason, RelPath, SideEntry, SideSource, Stamp,
};

use crate::index::{Child, TreeIndex};
use crate::location::Form;
use crate::repo::{Commit, GitProblem, MAX_BLOB, Repo};

/// How a side opens its repository: `Repo::open`, or `Repo::open_isolated` in tests.
type Opener = fn(&Path) -> Result<Repo, GitProblem>;

/// One revision of a repository as a compare side. The commit is pinned at open and moves
/// only on `refresh`; the tree index is built on first use and shared by every caller.
/// gix's `Repo` is not `Send` without its `parallel` feature, so each call reopens it (as
/// NV's `GixGit` does) instead of holding one.
pub struct GitTreeSide {
    path: PathBuf,
    opener: Opener,
    rev: String,
    form: Form,
    provider: ProviderId,
    commit: Mutex<Commit>,
    index: Mutex<Option<Arc<TreeIndex>>>,
    #[cfg(test)]
    builds: std::sync::atomic::AtomicUsize,
}

impl GitTreeSide {
    /// Opens the repository at `repo` and pins `rev` to its commit. Fails with `NotFound` for
    /// a missing repository or revision, `Unsupported` for a format or form it cannot read,
    /// `ReadError` for a damaged repository. Blocking: call off the UI thread.
    pub fn open(repo: &Path, rev: &str, form: Form) -> Result<GitTreeSide, Reason> {
        Self::with_opener(repo, Repo::open, rev, form)
    }

    /// As `open`, with only the repository's own config, for tests.
    #[cfg(any(test, feature = "test-support"))]
    pub fn open_isolated(repo: &Path, rev: &str, form: Form) -> Result<GitTreeSide, Reason> {
        Self::with_opener(repo, Repo::open_isolated, rev, form)
    }

    /// Opens `path` with `opener`, resolves `rev` and builds the side around it.
    fn with_opener(
        path: &Path,
        opener: Opener,
        rev: &str,
        form: Form,
    ) -> Result<GitTreeSide, Reason> {
        let provider = form.provider().map_err(|_| Reason::Unsupported)?;
        let repo = opener(path).map_err(GitProblem::reason)?;
        let commit = repo.resolve(rev).map_err(GitProblem::reason)?;
        Ok(GitTreeSide {
            path: path.to_path_buf(),
            opener,
            rev: rev.to_owned(),
            form,
            provider,
            commit: Mutex::new(commit),
            index: Mutex::new(None),
            #[cfg(test)]
            builds: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    /// The commit this side shows now.
    #[must_use]
    pub fn commit(&self) -> Commit {
        lock(&self.commit).clone()
    }

    /// The revision text this side was opened with.
    #[must_use]
    pub fn rev(&self) -> &str {
        &self.rev
    }

    /// The location form this side was opened with.
    #[must_use]
    pub fn form(&self) -> Form {
        self.form
    }

    /// Opens the repository again on this thread. Fails as `open` does.
    fn repo(&self) -> Result<Repo, Reason> {
        (self.opener)(&self.path).map_err(GitProblem::reason)
    }

    /// The tree index of the pinned commit, built under the lock on first use so concurrent
    /// callers share one build. Fails as `TreeIndex::build`; a failed build is retried later.
    fn index(&self, cancel: &CancelToken) -> Result<Arc<TreeIndex>, Reason> {
        // Lock order everywhere: index, then commit.
        let mut slot = lock(&self.index);
        if let Some(index) = slot.as_ref() {
            return Ok(Arc::clone(index));
        }
        let commit = self.commit();
        #[cfg(test)]
        self.builds
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let index = Arc::new(TreeIndex::build(&self.repo()?, commit, cancel)?);
        *slot = Some(Arc::clone(&index));
        Ok(index)
    }

    /// The index entry at `rel`. Fails with `NotFound` for an unknown or non-text path, or
    /// with the index build's reason.
    fn child(&self, rel: &RelPath) -> Result<Child, Reason> {
        let index = self.index(&CancelToken::new())?;
        let text = rel.as_path().as_str().ok_or(Reason::NotFound)?;
        index.child(text).cloned().ok_or(Reason::NotFound)
    }

    /// The object id the index holds for `rel`, for tests that damage the object store.
    #[cfg(test)]
    fn index_child_id(&self, rel: &str) -> Option<gix::ObjectId> {
        self.index(&CancelToken::new())
            .ok()?
            .child(rel)
            .map(|child| child.id)
    }
}

/// Locks `mutex`, taking the value back from a poisoned lock: every holder stores whole
/// values, so the data is never half-written.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `name` joined under `folder` (`""` is the root) as a row path, kept verbatim.
fn child_rel(folder: &str, name: &str) -> RelPath {
    let text = if folder.is_empty() {
        name.to_owned()
    } else {
        format!("{folder}/{name}")
    };
    RelPath::new(ProviderPath::from_bytes(text.as_bytes()))
}

/// Reads the blob `id` whole. Fails with `MissingBlob` when the object is absent,
/// `Unsupported` past `MAX_BLOB` and `ReadError` for any other error or a parser panic.
fn read_blob(repo: &Repo, id: gix::ObjectId) -> Result<Vec<u8>, Reason> {
    let local = repo.local();
    let found = catch_unwind(AssertUnwindSafe(|| {
        let object = local.try_find_object(id).map_err(|error| {
            if error.dominant_class() == Some(Class::NotFound) {
                Reason::MissingBlob
            } else {
                Reason::ReadError
            }
        })?;
        let object = object.ok_or(Reason::MissingBlob)?.detach();
        if object.kind != gix::objs::Kind::Blob {
            return Err(Reason::ReadError);
        }
        Ok(object.data)
    }));
    let data = found.map_err(|_| Reason::ReadError)??;
    if data.len() as u64 > MAX_BLOB {
        return Err(Reason::Unsupported);
    }
    Ok(data)
}

impl SideSource for GitTreeSide {
    fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// Feeds the children of `folder` from the index (built on the first call, honouring
    /// `cancel`). Fails with `NotFound` for a path that is not a folder.
    fn list(
        &self,
        folder: &RelPath,
        sink: &mut dyn FnMut(SideEntry) -> ControlFlow<()>,
        cancel: &CancelToken,
    ) -> Result<(), Reason> {
        let index = self.index(cancel)?;
        let folder = folder.as_path().as_str().ok_or(Reason::NotFound)?;
        let children = index.children(folder).ok_or(Reason::NotFound)?;
        for child in children {
            let rel = child_rel(folder, &child.name);
            let entry = SideEntry::new(rel, child.kind, child.size, None);
            if cancel.is_cancelled() || sink(entry).is_break() {
                break;
            }
        }
        Ok(())
    }

    /// Reads a file's stored bytes into memory. Links, folders, submodules and non-UTF-8
    /// paths are `Unsupported`, as is a blob over `MAX_BLOB` (refused before any read).
    fn open(&self, rel: &RelPath, _hint: ReadHint) -> Result<Box<dyn Read + Send>, Reason> {
        let child = self.child(rel)?;
        if child.kind != EntryKind::File || !child.readable || child.size > MAX_BLOB {
            return Err(Reason::Unsupported);
        }
        let data = read_blob(&self.repo()?, child.id)?;
        Ok(Box::new(Cursor::new(data)))
    }

    /// The indexed size; a commit has no modified time and no file id.
    fn stamp(&self, rel: &RelPath) -> Result<Stamp, Reason> {
        let child = self.child(rel)?;
        Ok(Stamp {
            size: child.size,
            modified: None,
            file_id: None,
        })
    }

    /// The blob id of a file, so two revisions compare without reading; `None` otherwise.
    fn object_id(&self, rel: &RelPath) -> Option<ObjectId> {
        let child = self.child(rel).ok()?;
        (child.kind == EntryKind::File)
            .then(|| ObjectId::from_bytes(child.id.as_slice()))
            .flatten()
    }

    fn file_id(&self, _rel: &RelPath) -> Option<FileIdentity> {
        None
    }

    /// `<commit hex>/<rel>`: stable for one commit, different after the revision moves.
    fn cache_path(&self, rel: &RelPath) -> ProviderPath {
        let hex: String = self
            .commit()
            .id
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        ProviderPath::from_text(&format!("{hex}/{}", rel.as_path().to_text()))
    }

    fn case_sensitive(&self) -> bool {
        true
    }

    fn read_only(&self) -> bool {
        true
    }

    fn may_have_unknown_size(&self) -> bool {
        false
    }

    /// Resolves the revision again; a new commit drops the index. A failed resolve (such as
    /// a deleted branch) keeps the pinned commit and its listing.
    fn refresh(&self) {
        let resolved = self
            .repo()
            .and_then(|repo| repo.resolve(&self.rev).map_err(GitProblem::reason));
        let fresh = match resolved {
            Ok(commit) => commit,
            Err(reason) => {
                tracing::warn!(
                    ?reason,
                    "git side: the revision no longer resolves; kept the pinned commit"
                );
                return;
            }
        };
        let mut index = lock(&self.index);
        let mut commit = lock(&self.commit);
        if *commit != fresh {
            *commit = fresh;
            *index = None;
        }
    }
}

#[cfg(test)]
#[path = "side_tests.rs"]
mod tests;
