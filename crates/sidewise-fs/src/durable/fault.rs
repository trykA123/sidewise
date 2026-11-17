//! `FaultDisk` (feature `test-support`): wraps `RealDisk`, numbers every op, fails a chosen op,
//! and `crash()` simulates power loss so journal tests can run fault and crash loops (D2).

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use super::{Disk, DiskFile, Meta, RealDisk, ReplaceError};

mod image;

use image::{Change, cut, snap, undo};

/// Which op breaks. Ops are numbered from 1 in call order: every `Disk` call and every
/// `DiskFile::write` or `sync` is one op; `flush` and reads from `open_read` are not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Every op runs.
    Pass,
    /// Op `n` fails with this kind and does nothing; later ops run.
    FailAt(u64, io::ErrorKind),
    /// Op `n` and every later op fail and do nothing, as after a power loss.
    CrashAt(u64),
}

/// One change and the folders still unsynced since it.
struct Pending {
    change: Change,
    dirs: Vec<PathBuf>,
}

/// A file written through the layer: where it is now (`None` once unlinked), its length,
/// and its length at the last `sync`.
struct Written {
    path: Option<PathBuf>,
    len: u64,
    synced: u64,
}

/// The op counter and what a crash would undo.
#[derive(Default)]
struct State {
    count: u64,
    crashed: bool,
    files: Vec<Written>,
    pending: Vec<Pending>,
}

/// The plan and state shared by the layer and its open files.
struct Shared {
    plan: Plan,
    state: Mutex<State>,
}

impl Shared {
    /// Counts one op and returns the state locked for it; fails as the plan says for this op,
    /// or when the layer crashed. A poisoned lock fails closed.
    fn tick(&self) -> io::Result<MutexGuard<'_, State>> {
        let mut state = self.lock()?;
        state.count += 1;
        let n = state.count;
        if state.crashed {
            return Err(io::Error::other(format!("disk crashed before op {n}")));
        }
        match self.plan {
            Plan::FailAt(at, kind) if at == n => {
                Err(io::Error::new(kind, format!("injected fault at op {n}")))
            }
            Plan::CrashAt(at) if at <= n => {
                state.crashed = true;
                Err(io::Error::other(format!("disk crashed at op {n}")))
            }
            _ => Ok(state),
        }
    }

    /// Locks the state without counting an op; a poisoned lock fails closed.
    fn lock(&self) -> io::Result<MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| io::Error::other("fault layer lock poisoned"))
    }
}

/// The fault and crash layer over `RealDisk`.
pub struct FaultDisk {
    inner: RealDisk,
    shared: Arc<Shared>,
}

/// A file opened through the layer; its writes and syncs are ops, tracked for `crash`.
struct FaultFile {
    inner: Box<dyn DiskFile>,
    shared: Arc<Shared>,
    id: usize,
}

impl Write for FaultFile {
    /// One op; adds the bytes written to the unsynced length.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self.shared.tick()?;
        let written = self.inner.write(buf)?;
        if let Some(file) = state.files.get_mut(self.id) {
            file.len += written as u64;
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl DiskFile for FaultFile {
    /// One op; on success the whole current length survives a crash.
    fn sync(&mut self) -> io::Result<()> {
        let mut state = self.shared.tick()?;
        self.inner.sync()?;
        if let Some(file) = state.files.get_mut(self.id) {
            file.synced = file.len;
        }
        Ok(())
    }
}

/// The folder that holds `path` (empty for a bare name).
fn parent(path: &Path) -> PathBuf {
    path.parent().map(Path::to_path_buf).unwrap_or_default()
}

/// `path` with its prefix `from` swapped for `to`; `None` when `path` is not at or under `from`.
fn rebase(path: &Path, from: &Path, to: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(from).ok()?;
    // `join("")` would add a trailing slash, which a file path must not have.
    Some(if rest.as_os_str().is_empty() {
        to.to_path_buf()
    } else {
        to.join(rest)
    })
}

impl State {
    /// Records a change that stays undoable until the folders of `paths` are synced.
    fn push(&mut self, change: Change, paths: &[&Path]) {
        let mut dirs: Vec<PathBuf> = paths.iter().map(|path| parent(path)).collect();
        dirs.dedup();
        self.pending.push(Pending { change, dirs });
    }

    /// Marks `dir` synced: changes with no unsynced folder left are durable and dropped.
    fn synced_dir(&mut self, dir: &Path) {
        for pending in &mut self.pending {
            pending.dirs.retain(|unsynced| unsynced != dir);
        }
        self.pending.retain(|pending| !pending.dirs.is_empty());
    }

    /// Moves tracked files at or under `from` to the same place under `to`.
    fn moved(&mut self, from: &Path, to: &Path) {
        for file in &mut self.files {
            if let Some(moved) = file.path.as_deref().and_then(|p| rebase(p, from, to)) {
                file.path = Some(moved);
            }
        }
    }

    /// Forgets the place of tracked files at or under `path`: they were unlinked.
    fn unlinked(&mut self, path: &Path) {
        for file in &mut self.files {
            if file.path.as_deref().is_some_and(|p| p.starts_with(path)) {
                file.path = None;
            }
        }
    }

    /// The last synced length of the file the layer tracks at `path`, if any.
    fn synced_len(&self, path: &Path) -> Option<u64> {
        let file = self
            .files
            .iter()
            .find(|f| f.path.as_deref() == Some(path))?;
        Some(file.synced)
    }

    /// Starts tracking a file opened for writing at `path` with `len` bytes already durable.
    fn track(&mut self, path: &Path, len: u64) -> usize {
        let known = self
            .files
            .iter()
            .position(|f| f.path.as_deref() == Some(path));
        if let Some(id) = known {
            return id;
        }
        self.files.push(Written {
            path: Some(path.to_path_buf()),
            len,
            synced: len,
        });
        self.files.len() - 1
    }
}

impl FaultDisk {
    /// A layer over `inner` that breaks ops as `plan` says.
    pub fn new(inner: RealDisk, plan: Plan) -> Self {
        Self {
            inner,
            shared: Arc::new(Shared {
                plan,
                state: Mutex::new(State::default()),
            }),
        }
    }

    /// The number of ops counted so far, failed ones included; after a `Pass` run, its length.
    pub fn ops(&self) -> u64 {
        self.shared.lock().map_or(0, |state| state.count)
    }

    /// Simulates power loss: every later op fails; each file written through the layer is cut
    /// to its last synced length; every create, rename or remove whose folder was not synced
    /// since is undone, newest first. Fails when the disk refuses a restore step.
    pub fn crash(&self) -> io::Result<()> {
        let mut state = self.shared.lock()?;
        state.crashed = true;
        for file in &state.files {
            if let Some(path) = file.path.as_deref().filter(|_| file.len > file.synced) {
                cut(path, file.synced)?;
            }
        }
        while let Some(pending) = state.pending.pop() {
            undo(pending.change)?;
        }
        state.files.clear();
        Ok(())
    }

    /// Wraps a file the inner disk opened, tracked as `id` in the state.
    fn wrap(&self, id: usize, inner: Box<dyn DiskFile>) -> Box<dyn DiskFile> {
        Box::new(FaultFile {
            inner,
            shared: Arc::clone(&self.shared),
            id,
        })
    }

    /// Runs an unlinking op (`remove_file` or `remove_dir_all`) and records it for `crash`.
    /// Fails closed, doing nothing, when the old bytes cannot be read first.
    fn removing(&self, path: &Path, op: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<()> {
        let mut state = self.shared.tick()?;
        let old = snap(path, &|p| state.synced_len(p))?;
        op(path)?;
        state.unlinked(path);
        let change = Change::Removed {
            path: path.to_path_buf(),
            old,
        };
        state.push(change, &[path]);
        Ok(())
    }
}

impl Disk for FaultDisk {
    fn create_new(&self, path: &Path) -> io::Result<Box<dyn DiskFile>> {
        let mut state = self.shared.tick()?;
        let file = self.inner.create_new(path)?;
        // A record still at this name is stale: the name was free, so that file is gone.
        state.unlinked(path);
        state.push(Change::Created(path.to_path_buf()), &[path]);
        let id = state.track(path, 0);
        Ok(self.wrap(id, file))
    }

    fn open_append(&self, path: &Path) -> io::Result<Box<dyn DiskFile>> {
        let mut state = self.shared.tick()?;
        let file = self.inner.open_append(path)?;
        let len = fs::symlink_metadata(path)?.len();
        let id = state.track(path, len);
        Ok(self.wrap(id, file))
    }

    fn open_read(&self, path: &Path) -> io::Result<Box<dyn Read + Send>> {
        let _state = self.shared.tick()?;
        self.inner.open_read(path)
    }

    fn meta(&self, path: &Path) -> io::Result<Option<Meta>> {
        let _state = self.shared.tick()?;
        self.inner.meta(path)
    }

    fn set_modified(&self, path: &Path, ms: i64) -> io::Result<()> {
        let _state = self.shared.tick()?;
        self.inner.set_modified(path, ms)
    }

    fn copy_mode(&self, from: &Path, to: &Path) -> io::Result<()> {
        let _state = self.shared.tick()?;
        self.inner.copy_mode(from, to)
    }

    /// The inner replace syncs the folder on success, so that sync covers every change
    /// pending in it; `NotDurable` stays undoable; `TargetMoved` is a rename to `old`.
    fn replace(&self, staged: &Path, target: &Path, old: &Path) -> Result<(), ReplaceError> {
        let mut state = self.shared.tick().map_err(ReplaceError::Unchanged)?;
        let before = match fs::symlink_metadata(target) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            _ => Some(snap(target, &|p| state.synced_len(p)).map_err(ReplaceError::Unchanged)?),
        };
        let swapped = self.inner.replace(staged, target, old);
        let change = match &swapped {
            Err(ReplaceError::Unchanged(_)) => return swapped,
            Err(ReplaceError::TargetMoved(_)) => {
                state.moved(target, old);
                let (from, to) = (target.to_path_buf(), old.to_path_buf());
                state.push(Change::Renamed { from, to }, &[target, old]);
                return swapped;
            }
            Ok(()) | Err(ReplaceError::NotDurable(_)) => Change::Replaced {
                staged: staged.to_path_buf(),
                target: target.to_path_buf(),
                old: before,
            },
        };
        state.unlinked(target);
        state.moved(staged, target);
        state.push(change, &[target]);
        if swapped.is_ok() {
            state.synced_dir(&parent(target));
        }
        swapped
    }

    fn rename_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        let mut state = self.shared.tick()?;
        self.inner.rename_new(from, to)?;
        state.moved(from, to);
        let change = Change::Renamed {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        };
        state.push(change, &[from, to]);
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.removing(path, |path| self.inner.remove_file(path))
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        let mut state = self.shared.tick()?;
        self.inner.create_dir(path)?;
        state.push(Change::Created(path.to_path_buf()), &[path]);
        Ok(())
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.removing(path, |path| self.inner.remove_dir_all(path))
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<String>> {
        let _state = self.shared.tick()?;
        self.inner.list(dir)
    }

    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        let mut state = self.shared.tick()?;
        self.inner.sync_dir(dir)?;
        state.synced_dir(dir);
        Ok(())
    }
}
