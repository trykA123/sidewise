//! An in-memory folder index of one commit's whole tree, built in one traversal with gix.
//! Read-only; never starts a program. The Git side lists and looks up entries from it.

use std::collections::{HashMap, VecDeque};
use std::ops::ControlFlow;
use std::panic::{AssertUnwindSafe, catch_unwind};

use gix::bstr::{BStr, BString, ByteSlice, ByteVec};
use gix::error::Class;
use gix::objs::tree::{EntryKind as GitKind, EntryRef};
use gix::traverse::tree::{Visit, visit::Action};
use sidewise_engine::model::{CancelToken, EntryKind, Reason};

use crate::repo::{Commit, Repo};

/// Most tree entries (files, folders, links, submodules) one index holds.
pub const MAX_ENTRIES: usize = 1_000_000;
/// Most path components of one entry.
pub const MAX_DEPTH: usize = 128;
/// Most bytes of one entry's full path (16 KiB).
pub const MAX_PATH_BYTES: usize = 16 * 1024;
/// How many entries pass between two looks at the cancel token.
const CANCEL_EVERY: usize = 1024;

/// One entry of a folder.
#[derive(Clone, Debug)]
pub struct Child {
    /// The entry name; a non-UTF-8 name is shown lossy.
    pub name: String,
    pub kind: EntryKind,
    /// Bytes of a file or link target; 0 for folders and submodules.
    pub size: u64,
    /// The object id as stored in the tree.
    pub id: gix::ObjectId,
    /// False when the full path is not UTF-8, so the entry cannot be read by name.
    pub readable: bool,
}

/// Every folder of a commit's tree with its children, sorted by name bytes.
pub struct TreeIndex {
    /// Keys are `/`-joined folder paths; `""` is the root. Every folder is present.
    folders: HashMap<String, Vec<Child>>,
    pub commit: Commit,
}

impl TreeIndex {
    /// Indexes the whole tree of `commit` in one traversal. Fails with `Stopped` on cancel,
    /// `Unsupported` past a cap or on a hostile name, `MissingBlob` for an absent object and
    /// `ReadError` for damaged data or a parser panic.
    pub fn build(repo: &Repo, commit: Commit, cancel: &CancelToken) -> Result<TreeIndex, Reason> {
        let local = repo.local();
        let built = catch_unwind(AssertUnwindSafe(|| {
            collect_tree(&local, commit.tree, cancel)
        }));
        let folders = built.map_err(|_| Reason::ReadError)??;
        Ok(TreeIndex { folders, commit })
    }

    /// The children of `folder` (`""` is the root), or `None` when it is not a folder.
    #[must_use]
    pub fn children(&self, folder: &str) -> Option<&[Child]> {
        self.folders.get(folder).map(Vec::as_slice)
    }

    /// The entry at `rel` (a `/`-joined path), found by binary search in its folder.
    #[must_use]
    pub fn child(&self, rel: &str) -> Option<&Child> {
        let (folder, name) = rel.rsplit_once('/').unwrap_or(("", rel));
        let children = self.children(folder)?;
        let at = children
            .binary_search_by(|child| child.name.as_str().cmp(name))
            .ok()?;
        children.get(at)
    }
}

/// Walks the tree `tree` depth-first and returns its folders. Errors as `TreeIndex::build`.
fn collect_tree(
    repo: &gix::Repository,
    tree: gix::ObjectId,
    cancel: &CancelToken,
) -> Result<HashMap<String, Vec<Child>>, Reason> {
    let tree = repo
        .find_tree(tree)
        .map_err(|error| object_problem(&error))?;
    let mut collect = Collect::new(repo, cancel);
    let walked = tree.traverse().depthfirst(&mut collect);
    if let Some(reason) = collect.stop {
        return Err(reason);
    }
    walked.map_err(|_| Reason::ReadError)?;
    Ok(collect.finish())
}

/// Maps an object lookup error: absence is `MissingBlob`, anything else `ReadError`.
fn object_problem(error: &gix::Error) -> Reason {
    match error.dominant_class() {
        Some(Class::NotFound) => Reason::MissingBlob,
        _ => Reason::ReadError,
    }
}

/// The folder whose entries the traversal is visiting now.
struct Folder {
    key: String,
    depth: usize,
    readable: bool,
}

/// The traversal delegate: tracks paths like gix's `Recorder` but keeps only pending folder
/// paths, never every entry path, and checks every cap before an entry is stored.
struct Collect<'a> {
    repo: &'a gix::Repository,
    cancel: &'a CancelToken,
    pending: VecDeque<BString>,
    path: BString,
    folder: Folder,
    open: Vec<Child>,
    folders: HashMap<String, Vec<Child>>,
    records: usize,
    stop: Option<Reason>,
}

impl<'a> Collect<'a> {
    /// A delegate positioned at the root folder.
    fn new(repo: &'a gix::Repository, cancel: &'a CancelToken) -> Self {
        let root = Folder {
            key: String::new(),
            depth: 0,
            readable: true,
        };
        Collect {
            repo,
            cancel,
            pending: VecDeque::new(),
            path: BString::default(),
            folder: root,
            open: Vec::new(),
            folders: HashMap::new(),
            records: 0,
            stop: None,
        }
    }

    /// Moves the visited folder's buffered children into the map and makes `self.path` current.
    fn enter_folder(&mut self) {
        let depth = if self.path.is_empty() {
            0
        } else {
            self.path.find_iter("/").count() + 1
        };
        let next = Folder {
            key: self.path.to_str_lossy().into_owned(),
            depth,
            readable: self.path.to_str().is_ok(),
        };
        let done = std::mem::replace(&mut self.folder, next);
        let children = std::mem::take(&mut self.open);
        self.folders.entry(done.key).or_default().extend(children);
    }

    /// Flushes the last folder, sorts every folder by name bytes and returns them.
    fn finish(mut self) -> HashMap<String, Vec<Child>> {
        self.path.clear();
        self.enter_folder();
        for children in self.folders.values_mut() {
            children.sort_by(|left, right| left.name.cmp(&right.name));
        }
        self.folders
    }

    /// Stores the entry or records why the traversal must stop, and says whether to go on.
    fn visit(&mut self, entry: &EntryRef<'_>) -> Action {
        match self.record(entry) {
            Ok(()) => ControlFlow::Continue(true),
            Err(reason) => {
                self.stop = Some(reason);
                ControlFlow::Break(())
            }
        }
    }

    /// Checks cancel and the caps, reads the size, then stores the entry at `self.path`.
    fn record(&mut self, entry: &EntryRef<'_>) -> Result<(), Reason> {
        if self.records.is_multiple_of(CANCEL_EVERY) && self.cancel.is_cancelled() {
            return Err(Reason::Stopped);
        }
        self.records += 1;
        let deep = self.folder.depth >= MAX_DEPTH || self.path.len() > MAX_PATH_BYTES;
        if self.records > MAX_ENTRIES || deep || !plain_name(entry.filename) {
            return Err(Reason::Unsupported);
        }
        let (kind, size) = match entry.mode.kind() {
            GitKind::Tree => (EntryKind::Folder, 0),
            GitKind::Blob | GitKind::BlobExecutable => (EntryKind::File, self.size(entry.oid)?),
            GitKind::Link => (EntryKind::Link, self.size(entry.oid)?),
            GitKind::Commit => (EntryKind::Submodule, 0),
        };
        if kind == EntryKind::Folder {
            let key = self.path.to_str_lossy().into_owned();
            self.folders.entry(key).or_default();
        }
        self.open.push(Child {
            name: entry.filename.to_str_lossy().into_owned(),
            kind,
            size,
            id: entry.oid.to_owned(),
            readable: self.folder.readable && entry.filename.to_str().is_ok(),
        });
        Ok(())
    }

    /// The stored size of object `id` from its header, in bytes, without reading it.
    fn size(&self, id: &gix::oid) -> Result<u64, Reason> {
        match self.repo.try_find_header(id) {
            Ok(Some(header)) => Ok(header.size()),
            Ok(None) => Err(Reason::MissingBlob),
            Err(_) => Err(Reason::ReadError),
        }
    }

    /// Appends `name` to the current path, as `Recorder` does.
    fn push_element(&mut self, name: &BStr) {
        if name.is_empty() {
            return;
        }
        if !self.path.is_empty() {
            self.path.push(b'/');
        }
        self.path.push_str(name);
    }
}

/// False for a name Git itself refuses (empty, `.`, `..`, or with `/` or NUL), which would
/// make two paths look the same.
fn plain_name(name: &BStr) -> bool {
    !(name.is_empty() || name == "." || name == ".." || name.contains(&b'/') || name.contains(&0))
}

impl Visit for Collect<'_> {
    fn pop_back_tracked_path_and_set_current(&mut self) {
        self.path = self.pending.pop_back().unwrap_or_default();
        self.enter_folder();
    }

    fn pop_front_tracked_path_and_set_current(&mut self) {
        self.path = self.pending.pop_front().unwrap_or_default();
        self.enter_folder();
    }

    fn push_back_tracked_path_component(&mut self, component: &BStr) {
        self.push_element(component);
        self.pending.push_back(self.path.clone());
    }

    fn push_path_component(&mut self, component: &BStr) {
        self.push_element(component);
    }

    fn pop_path_component(&mut self) {
        if let Some(at) = self.path.rfind_byte(b'/') {
            self.path.truncate(at);
        } else {
            self.path.clear();
        }
    }

    fn visit_tree(&mut self, entry: &EntryRef<'_>) -> Action {
        self.visit(entry)
    }

    fn visit_nontree(&mut self, entry: &EntryRef<'_>) -> Action {
        self.visit(entry)
    }
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
