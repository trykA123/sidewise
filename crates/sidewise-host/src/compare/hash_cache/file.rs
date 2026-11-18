//! The file-backed hash cache: one process per profile writes, the others only read.
//! Opens, locks and replays `hashes.v1`; the compare engine reads it through `HashCache`.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use navo_compare::model::{ContentKey, HashCache};

use super::compact::rewrite;
use super::record::{self, Digest};

/// How a store keeps its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    /// Holds the profile lock and owns the file.
    Writer,
    /// Another process writes; this one read the file at open and keeps puts in memory.
    Reader,
    /// No file at all.
    Memory,
}

/// Content hashes keyed by file digest, kept in memory and, for the writer, in a file.
pub struct FileHashes {
    path: Option<PathBuf>,
    state: Mutex<State>,
}

/// One cached hash and its write order (`seq`, rising), used when rewriting.
pub(super) struct Row {
    pub(super) hash: Digest,
    pub(super) seq: u64,
}

struct State {
    rows: HashMap<Digest, Row>,
    writer: Option<Writer>,
}

/// Present only in the writer: the held lock on `<path>.lock`.
struct Writer {
    _lock: File,
}

impl FileHashes {
    /// Opens the cache at `path`. Never fails: problems are logged and the store falls back to
    /// reader or memory mode.
    pub fn open(path: &Path) -> Self {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            && let Err(error) = fs::create_dir_all(parent)
        {
            tracing::warn!(%error, path = %parent.display(), "hash cache folder: memory only");
            return Self::memory();
        }
        let mut writer = take_lock(path);
        let replay = match fs::read(path) {
            Ok(bytes) => record::replay(&bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => record::replay(&[]),
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "hash cache unreadable");
                record::Replay::default()
            }
        };
        let mut rows = HashMap::with_capacity(replay.rows.len());
        for (seq, (key, hash)) in (0u64..).zip(replay.rows) {
            rows.insert(key, Row { hash, seq });
        }
        let damaged = !replay.header_ok || replay.bad > 0 || replay.torn;
        if writer.is_some()
            && damaged
            && let Err(error) = rewrite(path, &rows)
        {
            tracing::warn!(%error, path = %path.display(), "hash cache rewrite failed; not writing");
            writer = None;
        }
        Self {
            path: Some(path.to_path_buf()),
            state: Mutex::new(State { rows, writer }),
        }
    }

    /// A store without a file.
    pub fn memory() -> Self {
        Self {
            path: None,
            state: Mutex::new(State {
                rows: HashMap::new(),
                writer: None,
            }),
        }
    }

    /// Whether this store writes the file, only reads it, or has none.
    pub fn mode(&self) -> CacheMode {
        match (&self.path, &self.state().writer) {
            (None, _) => CacheMode::Memory,
            (Some(_), Some(_)) => CacheMode::Writer,
            (Some(_), None) => CacheMode::Reader,
        }
    }

    /// How many hashes the store holds.
    pub fn len(&self) -> usize {
        self.state().rows.len()
    }

    /// True when the store holds no hash.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Locks the state; a panic in another thread leaves plain data, so poison is ignored.
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Takes the profile lock on `<path>.lock`; `None` means another process writes, or the lock
/// could not be taken (logged), so this store only reads.
fn take_lock(path: &Path) -> Option<Writer> {
    let lock_path = with_suffix(path, ".lock");
    let result = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(TryLockError::Error)
        .and_then(|file| file.try_lock().map(|()| file));
    match result {
        Ok(file) => Some(Writer { _lock: file }),
        Err(TryLockError::WouldBlock) => None,
        Err(TryLockError::Error(error)) => {
            tracing::warn!(%error, path = %lock_path.display(), "hash cache lock failed; reading only");
            None
        }
    }
}

/// Returns `path` with `suffix` appended to its file name (`hashes.v1` + `.lock`).
pub(super) fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

impl HashCache for FileHashes {
    fn get(&self, key: &ContentKey) -> Option<[u8; 32]> {
        let digest = record::key_digest(key)?;
        self.state().rows.get(&digest).map(|row| row.hash)
    }

    // H2a2
    fn put(&self, _key: &ContentKey, _hash: [u8; 32]) {}
}
