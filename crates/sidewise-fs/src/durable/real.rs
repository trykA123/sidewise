//! `RealDisk`: the `Disk` trait on the OS. Unix is built here; elsewhere every call
//! returns `Unsupported` until the Windows module lands (D3), so no write runs half-built.

use std::fs::File;
use std::io;

#[cfg(unix)]
use std::{
    fs::OpenOptions,
    io::Read,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use super::{Disk, DiskFile, Meta, ReplaceError};

/// The OS file system.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealDisk;

impl super::DiskFile for File {
    /// `fsync`: data and metadata (size, mode, times), not only data.
    fn sync(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}

/// Converts a file time to ms since the Unix epoch; `None` past the `i64` range.
#[cfg(unix)]
fn to_ms(time: SystemTime) -> Option<i64> {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_millis()).ok(),
        Err(before) => i64::try_from(before.duration().as_millis())
            .ok()
            .map(|ms| -ms),
    }
}

/// Converts ms since the Unix epoch to a file time; fails with `InvalidInput` out of range.
#[cfg(unix)]
fn from_ms(ms: i64) -> io::Result<SystemTime> {
    let span = Duration::from_millis(ms.unsigned_abs());
    let time = if ms >= 0 {
        UNIX_EPOCH.checked_add(span)
    } else {
        UNIX_EPOCH.checked_sub(span)
    };
    time.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "time out of range"))
}

/// The folder that holds `path`; fails with `InvalidInput` for a root or a bare name.
#[cfg(unix)]
fn parent(path: &Path) -> io::Result<&Path> {
    path.parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent folder"))
}

#[cfg(unix)]
impl Disk for RealDisk {
    fn create_new(&self, path: &Path) -> io::Result<Box<dyn DiskFile>> {
        Ok(Box::new(
            OpenOptions::new().write(true).create_new(true).open(path)?,
        ))
    }

    fn open_append(&self, path: &Path) -> io::Result<Box<dyn DiskFile>> {
        Ok(Box::new(OpenOptions::new().append(true).open(path)?))
    }

    fn open_read(&self, path: &Path) -> io::Result<Box<dyn Read + Send>> {
        Ok(Box::new(File::open(path)?))
    }

    fn meta(&self, path: &Path) -> io::Result<Option<Meta>> {
        let found = match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            found => found?,
        };
        if !(found.is_file() || found.is_dir()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a file or folder",
            ));
        }
        Ok(Some(Meta {
            size: found.len(),
            modified_ms: found.modified().ok().and_then(to_ms),
            read_only: found.permissions().readonly(),
            identity: Some(crate::native::identity::from_metadata(&found)),
        }))
    }

    fn set_modified(&self, path: &Path, ms: i64) -> io::Result<()> {
        File::open(path)?.set_modified(from_ms(ms)?)
    }

    fn copy_mode(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::set_permissions(to, std::fs::symlink_metadata(from)?.permissions())
    }

    /// `rename`, then sync the folder. A failed sync after the rename is `Unchanged`:
    /// the swap is not durable, so the caller rolls back by hash.
    fn replace(&self, staged: &Path, target: &Path, _old: &Path) -> Result<(), ReplaceError> {
        let dir = parent(target).map_err(ReplaceError::Unchanged)?;
        std::fs::rename(staged, target).map_err(ReplaceError::Unchanged)?;
        self.sync_dir(dir).map_err(ReplaceError::Unchanged)
    }

    #[cfg(target_os = "linux")]
    fn rename_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        use rustix::fs::{CWD, RenameFlags, renameat_with};
        renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).map_err(|errno| {
            if errno == rustix::io::Errno::EXIST {
                io::ErrorKind::AlreadyExists.into()
            } else {
                io::Error::from(errno)
            }
        })
    }

    /// No atomic no-replace rename is wired outside Linux: refuse rather than race.
    #[cfg(not(target_os = "linux"))]
    fn rename_new(&self, _from: &Path, _to: &Path) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let name = entry?.file_name().into_string();
            names.push(
                name.map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "non-UTF-8 name"))?,
            );
        }
        names.sort_unstable();
        Ok(names)
    }

    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        File::open(dir)?.sync_all()
    }
}

/// Until D3, every Windows call fails closed with `Unsupported`.
#[cfg(not(unix))]
impl super::Disk for RealDisk {
    fn create_new(&self, _: &std::path::Path) -> io::Result<Box<dyn super::DiskFile>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn open_append(&self, _: &std::path::Path) -> io::Result<Box<dyn super::DiskFile>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn open_read(&self, _: &std::path::Path) -> io::Result<Box<dyn io::Read + Send>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn meta(&self, _: &std::path::Path) -> io::Result<Option<super::Meta>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn set_modified(&self, _: &std::path::Path, _: i64) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn copy_mode(&self, _: &std::path::Path, _: &std::path::Path) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn replace(
        &self,
        _: &std::path::Path,
        _: &std::path::Path,
        _: &std::path::Path,
    ) -> Result<(), super::ReplaceError> {
        Err(super::ReplaceError::Unchanged(
            io::ErrorKind::Unsupported.into(),
        ))
    }
    fn rename_new(&self, _: &std::path::Path, _: &std::path::Path) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn remove_file(&self, _: &std::path::Path) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn create_dir(&self, _: &std::path::Path) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn remove_dir_all(&self, _: &std::path::Path) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn list(&self, _: &std::path::Path) -> io::Result<Vec<String>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn sync_dir(&self, _: &std::path::Path) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
