//! File helpers shared by the job writer and the rollback: temp names beside a target (NC144),
//! a writer that hashes what it writes, and hashed copies and reads through a `Disk`.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use navo_core::TEMP_SUFFIX;
use sidewise_fs::durable::{Disk, DiskFile};

use crate::record::JobId;

/// Which temp of an item: the staged new bytes, a restore copy, or the old target moved aside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Temp {
    Staged,
    Restore,
    Old,
}

/// The temp path `.sw-<job>-<item>[-r|-old].navo-part` in the folder of `target`.
/// Fails with `InvalidInput` when the target has no parent folder.
pub(crate) fn temp_path(target: &Path, job: &JobId, item: u32, temp: Temp) -> io::Result<PathBuf> {
    let dir = target
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no folder"))?;
    let tag = match temp {
        Temp::Staged => "",
        Temp::Restore => "-r",
        Temp::Old => "-old",
    };
    Ok(dir.join(format!(".sw-{}-{item}{tag}{TEMP_SUFFIX}", job.as_str())))
}

/// The backup path of an item inside its job folder, `<item>.bak`.
pub(crate) fn backup_path(job_dir: &Path, item: u32) -> PathBuf {
    job_dir.join(format!("{item}.bak"))
}

/// A writer over a disk file that counts and hashes every byte it writes, and keeps the
/// last OS error so a caller's text error can be mapped back to it (disk full, in use).
pub(crate) struct Hashing {
    pub(crate) file: Box<dyn DiskFile>,
    pub(crate) hasher: blake3::Hasher,
    pub(crate) len: u64,
    pub(crate) error: Option<io::Error>,
}

impl Hashing {
    /// Wraps a freshly created, empty file.
    pub(crate) fn new(file: Box<dyn DiskFile>) -> Self {
        Self {
            file,
            hasher: blake3::Hasher::new(),
            len: 0,
            error: None,
        }
    }
}

impl Write for Hashing {
    /// Writes to the file, then hashes and counts only the bytes it took.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.file.write(buf) {
            Ok(written) => {
                let taken = buf.get(..written).unwrap_or(buf);
                self.hasher.update(taken);
                self.len += written as u64;
                Ok(written)
            }
            Err(error) => {
                self.error = Some(io::Error::new(error.kind(), error.to_string()));
                Err(error)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

/// Copies `from` into a new file at `to`, runs `finish` (set times or mode), syncs the copy,
/// and returns its length and blake3 hash. Fails when `to` exists, `from` is a symlink or
/// special file, or any step fails.
pub(crate) fn copy_hashed(
    disk: &dyn Disk,
    from: &Path,
    to: &Path,
    finish: impl FnOnce() -> io::Result<()>,
) -> io::Result<(u64, [u8; 32])> {
    let mut source = disk.open_read(from)?;
    let mut out = Hashing::new(disk.create_new(to)?);
    io::copy(&mut source, &mut out)?;
    finish()?;
    out.file.sync()?;
    Ok((out.len, *out.hasher.finalize().as_bytes()))
}

/// The blake3 hash of the file at `path`; `None` when it is missing.
/// Fails on a symlink, a special file or a read error.
pub(crate) fn hash_file(disk: &dyn Disk, path: &Path) -> io::Result<Option<[u8; 32]>> {
    let mut source = match disk.open_read(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        opened => opened?,
    };
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        let read = source.read(&mut buf)?;
        if read == 0 {
            return Ok(Some(*hasher.finalize().as_bytes()));
        }
        hasher.update(buf.get(..read).unwrap_or(&buf));
    }
}

/// Removes the file at `path` if it is there; returns whether it removed one.
pub(crate) fn remove_if_present(disk: &dyn Disk, path: &Path) -> io::Result<bool> {
    match disk.remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
