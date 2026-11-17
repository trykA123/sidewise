//! Tests of overwrites on volumes without stable file ids (exFAT, FAT; NC202): there the target
//! is re-hashed just before the replace instead of matched by identity, in a job, an undo, a
//! rollback and the crash loops. A volume with stable ids still refuses an unknown identity.
// Every platform; with SIDEWISE_NO_IDS_DIR set to a folder on exFAT, the real-disk case runs there.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sidewise_fs::durable::{Disk, DiskFile, Meta, RealDisk, ReplaceError};

use crate::{Expect, JobKind, Journal, WriteError};

const OLD: &[u8] = b"old bytes";
/// Other bytes of the same length as `OLD`.
const SWAP: &[u8] = b"his bytes";
const NEW: &[u8] = b"the new bytes";
/// On a 2 s boundary, so exFAT's write-time granularity keeps it exact.
const OLD_MS: i64 = 1_700_000_002_000;

/// A disk over `inner` that reports no identity, like exFAT. `stable` says whether the volume
/// claims stable ids anyway (an unknown identity there must still be refused). With `swap`,
/// the first `meta` of that path after a `.bak` was made rewrites it with `SWAP` and puts its
/// modified time back: a same-size, same-time edit between backup and replace.
struct NoIds {
    inner: Arc<dyn Disk>,
    stable: bool,
    swap: Option<PathBuf>,
    armed: Mutex<bool>,
}

/// `inner` on a volume without stable ids.
fn no_ids(inner: Arc<dyn Disk>) -> Arc<dyn Disk> {
    Arc::new(NoIds {
        inner,
        stable: false,
        swap: None,
        armed: Mutex::new(false),
    })
}

impl NoIds {
    /// Rewrites the swap target once, if armed and `path` is it.
    fn maybe_swap(&self, path: &Path) -> io::Result<()> {
        let mut armed = self
            .armed
            .lock()
            .map_err(|_| io::Error::other("poisoned"))?;
        if *armed && self.swap.as_deref() == Some(path) {
            *armed = false;
            fs::write(path, SWAP)?;
            RealDisk.set_modified(path, OLD_MS)?;
        }
        Ok(())
    }
}

impl Disk for NoIds {
    fn create_new(&self, path: &Path) -> io::Result<Box<dyn DiskFile>> {
        if path.extension().is_some_and(|ext| ext == "bak") {
            *self
                .armed
                .lock()
                .map_err(|_| io::Error::other("poisoned"))? = true;
        }
        self.inner.create_new(path)
    }
    fn open_append(&self, path: &Path) -> io::Result<Box<dyn DiskFile>> {
        self.inner.open_append(path)
    }
    fn open_read(&self, path: &Path) -> io::Result<Box<dyn Read + Send>> {
        self.inner.open_read(path)
    }
    fn meta(&self, path: &Path) -> io::Result<Option<Meta>> {
        self.maybe_swap(path)?;
        Ok(self.inner.meta(path)?.map(|meta| Meta {
            identity: None,
            no_stable_ids: !self.stable,
            ..meta
        }))
    }
    fn set_modified(&self, path: &Path, ms: i64) -> io::Result<()> {
        self.inner.set_modified(path, ms)
    }
    fn copy_mode(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.inner.copy_mode(from, to)
    }
    fn replace(&self, staged: &Path, target: &Path, old: &Path) -> Result<(), ReplaceError> {
        self.inner.replace(staged, target, old)
    }
    fn rename_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.inner.rename_new(from, to)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.inner.remove_file(path)
    }
    fn create_dir(&self, path: &Path) -> io::Result<()> {
        self.inner.create_dir(path)
    }
    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.inner.remove_dir_all(path)
    }
    fn list(&self, dir: &Path) -> io::Result<Vec<String>> {
        self.inner.list(dir)
    }
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        self.inner.sync_dir(dir)
    }
}

/// A journal folder and `count` targets holding `OLD` at `OLD_MS`; the targets go in `at`
/// when given, else beside the journal.
struct Files {
    journal: tempfile::TempDir,
    _targets: tempfile::TempDir,
    targets: Vec<PathBuf>,
}

fn files(count: usize, at: Option<&Path>) -> Files {
    let journal = tempfile::tempdir().unwrap();
    let dir = match at {
        Some(at) => tempfile::tempdir_in(at).unwrap(),
        None => tempfile::tempdir().unwrap(),
    };
    let targets: Vec<PathBuf> = (0..count)
        .map(|i| dir.path().join(format!("{i}.txt")))
        .collect();
    for target in &targets {
        fs::write(target, OLD).unwrap();
        RealDisk.set_modified(target, OLD_MS).unwrap();
    }
    Files {
        journal,
        _targets: dir,
        targets,
    }
}

/// The bytes and modified time of `path`.
fn state(path: &Path) -> (Vec<u8>, Option<i64>) {
    let meta = RealDisk.meta(path).unwrap().unwrap();
    (fs::read(path).unwrap(), meta.modified_ms)
}

/// The names in the targets' folder that end in the temp suffix.
fn temps(files: &Files) -> Vec<String> {
    let dir = files.targets[0].parent().unwrap();
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(navo_core::TEMP_SUFFIX))
        .collect()
}

fn write_new(out: &mut dyn Write) -> Result<(), String> {
    out.write_all(NEW).map_err(|error| error.to_string())
}

/// Overwrites every target with `NEW` in one committed job, then undoes it, checking each step.
fn overwrite_then_undo(disk: Arc<dyn Disk>, files: &Files) {
    let journal = Journal::open_raw(files.journal.path(), disk).unwrap();
    let mut job = journal.begin(JobKind::CopyMany, "Copy", None).unwrap();
    for target in &files.targets {
        job.overwrite(target, Expect::default(), None, &mut write_new)
            .unwrap();
    }
    let id = job.commit().unwrap();
    for target in &files.targets {
        assert_eq!(fs::read(target).unwrap(), NEW);
    }
    assert_eq!(journal.undo(&id).unwrap(), files.targets);
    for target in &files.targets {
        assert_eq!(state(target), (OLD.to_vec(), Some(OLD_MS)), "undone");
    }
    assert!(temps(files).is_empty());
}

#[test]
fn an_unchanged_target_without_ids_is_overwritten_and_undone() {
    let files = files(2, None);
    overwrite_then_undo(no_ids(Arc::new(RealDisk)), &files);
}

#[test]
fn a_real_disk_overwrite_and_undo_work_and_exfat_reports_no_ids() {
    let at = std::env::var_os("SIDEWISE_NO_IDS_DIR").map(PathBuf::from);
    let files = files(1, at.as_deref());
    if at.is_some() {
        let meta = RealDisk.meta(&files.targets[0]).unwrap().unwrap();
        assert_eq!((meta.identity, meta.no_stable_ids), (None, true));
    }
    overwrite_then_undo(Arc::new(RealDisk), &files);
}

#[test]
fn a_same_size_same_time_swap_before_the_replace_is_refused_by_hash() {
    let files = files(1, None);
    let target = files.targets[0].clone();
    let disk = Arc::new(NoIds {
        inner: Arc::new(RealDisk),
        stable: false,
        swap: Some(target.clone()),
        armed: Mutex::new(false),
    });
    let journal = Journal::open_raw(files.journal.path(), disk).unwrap();
    let mut job = journal.begin(JobKind::Copy, "Copy", None).unwrap();
    let result = job.overwrite(&target, Expect::default(), None, &mut write_new);
    assert_eq!(result, Err(WriteError::Changed));
    assert_eq!(
        state(&target),
        (SWAP.to_vec(), Some(OLD_MS)),
        "the edit is kept"
    );
    assert!(temps(&files).is_empty());
}

#[test]
fn an_unknown_identity_on_a_volume_with_stable_ids_is_still_refused() {
    let files = files(1, None);
    let disk = Arc::new(NoIds {
        inner: Arc::new(RealDisk),
        stable: true,
        swap: None,
        armed: Mutex::new(false),
    });
    let journal = Journal::open_raw(files.journal.path(), disk).unwrap();
    let mut job = journal.begin(JobKind::Copy, "Copy", None).unwrap();
    let result = job.overwrite(&files.targets[0], Expect::default(), None, &mut write_new);
    assert_eq!(
        result,
        Err(WriteError::Failed("the file identity is unknown".into()))
    );
    assert_eq!(state(&files.targets[0]), (OLD.to_vec(), Some(OLD_MS)));
}

#[test]
fn a_failed_second_item_rolls_the_first_back_without_ids() {
    let files = files(2, None);
    let journal = Journal::open_raw(files.journal.path(), no_ids(Arc::new(RealDisk))).unwrap();
    let mut job = journal.begin(JobKind::CopyMany, "Copy", None).unwrap();
    job.overwrite(&files.targets[0], Expect::default(), None, &mut write_new)
        .unwrap();
    let mut fail = |_: &mut dyn Write| Err("no source".to_owned());
    let result = job.overwrite(&files.targets[1], Expect::default(), None, &mut fail);
    assert_eq!(result, Err(WriteError::Failed("no source".into())));
    for target in &files.targets {
        assert_eq!(state(target), (OLD.to_vec(), Some(OLD_MS)), "rolled back");
    }
    assert!(temps(&files).is_empty());
}

/// The job crash loops and the recovery at open, run on a volume without stable ids.
#[cfg(target_os = "linux")]
mod crash {
    use std::sync::Arc;

    use sidewise_fs::durable::{Disk, FaultDisk, Plan, RealDisk};

    use super::no_ids;
    use crate::Journal;
    use crate::job_crash_tests::{PATTERNS, assert_recoverable};
    use crate::job_tests::{Fixture, run};
    use crate::recover_crash_tests::check_settled;

    /// Crashes a two-item job at every op and pattern, then checks it and recovers it.
    fn loop_without_ids(commit: bool) {
        let pass = Arc::new(FaultDisk::new(RealDisk, Plan::Pass));
        let _ = run(no_ids(pass.clone()), &Fixture::new(2), commit);
        let ops = pass.ops();
        assert!(ops > 40, "the loop covers both items ({ops} ops)");
        for n in 1..=ops + 1 {
            for (name, keep, tail) in PATTERNS {
                let fixture = Fixture::new(2);
                let fault = Arc::new(FaultDisk::new(RealDisk, Plan::CrashAt(n)));
                let _ = run(no_ids(fault.clone()), &fixture, commit);
                fault.crash_with(keep, tail).unwrap();
                let case = format!("crash at op {n} of {ops}, {name}");
                assert_recoverable(&fixture, &case);
                let disk: Arc<dyn Disk> = no_ids(Arc::new(RealDisk));
                let (_journal, recovered) = Journal::open(&fixture.journal_dir(), disk).unwrap();
                assert_eq!((recovered.failed, recovered.damaged), (0, 0), "{case}");
                check_settled(&fixture, &case).unwrap();
            }
        }
    }

    #[test]
    fn a_crash_at_every_op_without_ids_is_recoverable_after_commit() {
        loop_without_ids(true);
    }

    #[test]
    fn a_crash_at_every_op_without_ids_is_recoverable_during_rollback() {
        loop_without_ids(false);
    }
}
