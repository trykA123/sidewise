//! `RealDisk` on Windows (NC146): every path goes verbatim (`\\?\`), `replace` is
//! `ReplaceFileW` with an `-old` backup name, and `rename_new` never replaces.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_READONLY, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
    FILE_WRITE_ATTRIBUTES,
};

use super::real::{from_ms, list_sorted, parent, refused, replace_failure, to_ms};
use super::{Disk, DiskFile, Meta, RealDisk, ReplaceError};

/// The verbatim form of an absolute path, so long names and trailing dots pass unchanged.
/// Fails with `InvalidInput` on a relative path or one with a NUL unit.
fn verbatim(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(refused("path is not absolute"));
    }
    let units: Vec<u16> = path.as_os_str().encode_wide().collect();
    if units.contains(&0) {
        return Err(refused("path holds a NUL"));
    }
    let wide = crate::native::verbatim::verbatim(&units);
    Ok(PathBuf::from(OsString::from_wide(&wide)))
}

/// The verbatim path as a NUL-terminated UTF-16 string for a Win32 call.
fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut units: Vec<u16> = verbatim(path)?.as_os_str().encode_wide().collect();
    units.push(0);
    Ok(units)
}

/// Opens a regular file without following a link (symlink or junction).
/// Fails with `InvalidInput` on a link or anything that is not a regular file.
fn open_regular(path: &Path, options: &mut OpenOptions) -> io::Result<File> {
    let file = options
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(verbatim(path)?)?;
    let kind = file.metadata()?.file_type();
    if kind.is_symlink() {
        return Err(refused("path is a link"));
    }
    if !kind.is_file() {
        return Err(refused("not a regular file"));
    }
    Ok(file)
}

/// `ReplaceFileW(target, staged, old)`: the target keeps its creation time, ACL and streams.
/// Fails with the OS error; `replace_failure` maps it.
#[allow(unsafe_code, reason = "ReplaceFileW is a Win32 call")]
fn replace_file(target: &[u16], staged: &[u16], old: &[u16]) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{REPLACEFILE_IGNORE_MERGE_ERRORS, ReplaceFileW};
    // SAFETY: the three buffers are NUL-terminated and live through the call; the
    // exclude and reserved pointers must be null.
    let ok = unsafe {
        ReplaceFileW(
            target.as_ptr(),
            staged.as_ptr(),
            old.as_ptr(),
            REPLACEFILE_IGNORE_MERGE_ERRORS,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// `MoveFileExW` with write-through and without `REPLACE_EXISTING`: fails with
/// `AlreadyExists` (error 80 or 183) when `to` exists.
#[allow(unsafe_code, reason = "MoveFileExW is a Win32 call")]
fn move_new(from: &[u16], to: &[u16]) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
    // SAFETY: both buffers are NUL-terminated and live through the call.
    let ok = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

impl Disk for RealDisk {
    fn create_new(&self, path: &Path) -> io::Result<Box<dyn DiskFile>> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(verbatim(path)?)?;
        Ok(Box::new(file))
    }

    fn open_append(&self, path: &Path) -> io::Result<Box<dyn DiskFile>> {
        Ok(Box::new(
            OpenOptions::new().append(true).open(verbatim(path)?)?,
        ))
    }

    fn open_read(&self, path: &Path) -> io::Result<Box<dyn Read + Send>> {
        Ok(Box::new(open_regular(path, OpenOptions::new().read(true))?))
    }

    /// Reads the link itself, never its target; a symlink or junction is refused.
    fn meta(&self, path: &Path) -> io::Result<Option<Meta>> {
        let path = verbatim(path)?;
        let found = match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            found => found?,
        };
        if !(found.is_file() || found.is_dir()) {
            return Err(refused("not a file or folder"));
        }
        Ok(Some(Meta {
            size: found.len(),
            modified_ms: found.modified().ok().and_then(to_ms),
            read_only: found.file_attributes() & FILE_ATTRIBUTE_READONLY != 0,
            identity: crate::native::native_identity(&path)?,
        }))
    }

    fn set_modified(&self, path: &Path, ms: i64) -> io::Result<()> {
        let mut options = OpenOptions::new();
        options.access_mode(FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES);
        open_regular(path, &mut options)?.set_modified(from_ms(ms)?)
    }

    fn copy_mode(&self, _from: &Path, _to: &Path) -> io::Result<()> {
        Ok(())
    }

    /// Both files must share a folder, so the swap stays on one volume. After a swap the
    /// `old` backup is removed; a failed removal leaves a temp that rollback deletes.
    fn replace(&self, staged: &Path, target: &Path, old: &Path) -> Result<(), ReplaceError> {
        let dir = parent(target).map_err(ReplaceError::Unchanged)?;
        if parent(staged).ok() != Some(dir) {
            return Err(ReplaceError::Unchanged(refused(
                "staged file is in another folder",
            )));
        }
        let names = || io::Result::Ok((wide(target)?, wide(staged)?, wide(old)?));
        let (target_w, staged_w, old_w) = names().map_err(ReplaceError::Unchanged)?;
        replace_file(&target_w, &staged_w, &old_w).map_err(replace_failure)?;
        if let Err(error) = verbatim(old).and_then(std::fs::remove_file) {
            tracing::warn!(path = %old.display(), %error, "replace: could not remove the old file");
        }
        Ok(())
    }

    fn rename_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        move_new(&wide(from)?, &wide(to)?)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(verbatim(path)?)
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir(verbatim(path)?)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(verbatim(path)?)
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<String>> {
        list_sorted(&verbatim(dir)?)
    }

    /// A no-op on Windows (NC146).
    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        Ok(())
    }
}
