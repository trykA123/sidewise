//! The rollback of one job (NC145), by hash and item by item in reverse order. `Job::abort`
//! runs it in process; recovery at open (J3) runs it on the records of a dead owner's log.

use std::io;
use std::path::{Path, PathBuf};

use navo_core::FileStamp;
use sidewise_fs::durable::{Disk, ReplaceError};

use crate::files::{Temp, backup_path, copy_hashed, hash_file, remove_if_present, temp_path};
use crate::record::{ItemMode, JobId, Record};

/// What a rollback did, counted in items, and whether the job's log was closed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rolled {
    /// Targets put back to their old bytes and modified time.
    pub restored: u32,
    /// Targets changed by someone else since the write: file and backup are both kept.
    pub kept: u32,
    /// Items that could not be rolled back now; the job stays open for the next start.
    pub failed: u32,
    /// True when `Aborted` was durably appended, so the job is complete.
    pub closed: bool,
}

/// One item as its records describe it.
struct Item<'a> {
    item: u32,
    target: &'a str,
    mode: ItemMode,
    new: Option<&'a FileStamp>,
    old: Option<&'a FileStamp>,
}

/// How one item ended.
enum End {
    /// Nothing of the job is left in the target.
    Clean,
    /// The old bytes were written back.
    Restored,
    /// The target holds bytes the job never wrote; it is left alone.
    Kept,
}

/// Rolls back every item of the job in `job_dir`, newest first; each item stands alone, so
/// one failure does not stop the others. Leaves the log as it is; `closed` is always false.
pub(crate) fn roll_back(
    disk: &dyn Disk,
    job_dir: &Path,
    job: &JobId,
    records: &[Record],
) -> Rolled {
    let mut rolled = Rolled::default();
    for item in items(records).iter().rev() {
        match roll_item(disk, job_dir, job, item) {
            Ok(End::Clean) => {}
            Ok(End::Restored) => rolled.restored += 1,
            Ok(End::Kept) => rolled.kept += 1,
            Err(_) => rolled.failed += 1,
        }
    }
    rolled
}

/// The items of a log in `Intent` order, with their `Staged` and `Backed` stamps.
fn items(records: &[Record]) -> Vec<Item<'_>> {
    let mut items = Vec::new();
    for record in records {
        match record {
            Record::Intent {
                item, target, mode, ..
            } => items.push(Item {
                item: *item,
                target,
                mode: *mode,
                new: None,
                old: None,
            }),
            Record::Staged { item, new } => {
                if let Some(found) = items.iter_mut().find(|it| it.item == *item) {
                    found.new = Some(new);
                }
            }
            Record::Backed { item, old } => {
                if let Some(found) = items.iter_mut().find(|it| it.item == *item) {
                    found.old = Some(old);
                }
            }
            _ => {}
        }
    }
    items
}

/// The files of one item: its target, its backup and two of its temps.
struct Places<'a> {
    target: &'a Path,
    backup: PathBuf,
    restore: PathBuf,
    old_temp: PathBuf,
}

/// Rolls back one item: removes its staged and restore temps, settles the target when a
/// backup was recorded, then removes the moved-aside old temp and syncs the folder.
/// Fails, leaving the rest for the next run, when a step fails or the backup does not verify.
fn roll_item(disk: &dyn Disk, job_dir: &Path, job: &JobId, item: &Item<'_>) -> io::Result<End> {
    if item.mode != ItemMode::Overwrite {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "only Overwrite items roll back in P1a",
        ));
    }
    let target = Path::new(item.target);
    let temp = |kind| temp_path(target, job, item.item, kind);
    let places = Places {
        target,
        backup: backup_path(job_dir, item.item),
        restore: temp(Temp::Restore)?,
        old_temp: temp(Temp::Old)?,
    };
    let mut removed = remove_if_present(disk, &temp(Temp::Staged)?)?;
    removed |= remove_if_present(disk, &places.restore)?;
    let end = match item.old {
        // With no durable backup the replace never ran: the target is untouched.
        None => End::Clean,
        Some(old) => settle(disk, item, old, &places)?,
    };
    removed |= remove_if_present(disk, &places.old_temp)?;
    if removed {
        sync_parent(disk, target)?;
    }
    Ok(end)
}

/// Puts the old bytes back when the target holds the new ones or is gone (NC145 step 2).
/// Fails when the backup does not hash to `old`, or a step fails.
fn settle(
    disk: &dyn Disk,
    item: &Item<'_>,
    old: &FileStamp,
    places: &Places<'_>,
) -> io::Result<End> {
    if hash_file(disk, &places.backup)? != Some(old.hash) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "backup does not verify",
        ));
    }
    let is_new = |hash: [u8; 32]| item.new.is_some_and(|new| new.hash == hash);
    match hash_file(disk, places.target)? {
        Some(hash) if hash == old.hash => Ok(End::Clean),
        Some(hash) if is_new(hash) => {
            restore_from(disk, old, places, true)?;
            Ok(End::Restored)
        }
        Some(_) => Ok(End::Kept),
        None if hash_file(disk, &places.old_temp)? == Some(old.hash) => {
            disk.rename_new(&places.old_temp, places.target)?;
            sync_parent(disk, places.target)?;
            Ok(End::Restored)
        }
        None => {
            restore_from(disk, old, places, false)?;
            Ok(End::Restored)
        }
    }
}

/// Copies the backup to the restore temp with the old modified time (and, over a `present`
/// target, its mode), syncs it, then replaces the target, or renames it into place with no
/// replace when the target is gone. Fails when the copy does not hash to `old`.
fn restore_from(
    disk: &dyn Disk,
    old: &FileStamp,
    places: &Places<'_>,
    present: bool,
) -> io::Result<()> {
    let (target, restore) = (places.target, places.restore.as_path());
    let (_, hash) = copy_hashed(disk, &places.backup, restore, || {
        if let Some(ms) = old.modified_ms {
            disk.set_modified(restore, ms)?;
        }
        if present {
            disk.copy_mode(target, restore)?;
        }
        Ok(())
    })?;
    if hash != old.hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "restore copy does not verify",
        ));
    }
    if present {
        return disk
            .replace(restore, target, &places.old_temp)
            .map_err(|error| match error {
                ReplaceError::Unchanged(error)
                | ReplaceError::NotDurable(error)
                | ReplaceError::TargetMoved(error) => error,
            });
    }
    disk.rename_new(restore, target)?;
    sync_parent(disk, target)
}

/// Syncs the folder that holds `path`.
fn sync_parent(disk: &dyn Disk, path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(dir) => disk.sync_dir(dir),
        None => Ok(()),
    }
}
