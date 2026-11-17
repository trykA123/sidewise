//! Temp-folder tests of the Windows `RealDisk` contract: long paths, the read-only
//! attribute, sharing locks on `replace`, junctions and the no-replace rename.
// Windows only; clippy sees `cfg(test)` on the module, so test unwraps stay allowed.
#![cfg(windows)]

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use super::{Disk, RealDisk, ReplaceError};

fn put(path: &Path, text: &str) {
    let mut file = RealDisk.create_new(path).unwrap();
    file.write_all(text.as_bytes()).unwrap();
    file.sync().unwrap();
}

fn read(path: &Path) -> String {
    let mut text = String::new();
    RealDisk
        .open_read(path)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

/// Builds a folder chain under `root` whose full path passes 300 characters.
fn deep_dir(root: &Path) -> PathBuf {
    let mut dir = root.to_path_buf();
    while dir.as_os_str().len() < 300 {
        dir.push("d".repeat(60));
        RealDisk.create_dir(&dir).unwrap();
    }
    dir
}

#[test]
fn every_step_works_on_a_path_past_300_characters() {
    let root = tempfile::tempdir().unwrap();
    let dir = deep_dir(root.path());
    let (staged, target, old) = (dir.join("s"), dir.join("t"), dir.join("o"));
    put(&target, "old");
    let mut log = RealDisk.open_append(&target).unwrap();
    log.write_all(b"+").unwrap();
    log.sync().unwrap();
    drop(log);
    put(&staged, "new");
    RealDisk.set_modified(&staged, 1_600_000_000_000).unwrap();
    let meta = RealDisk.meta(&staged).unwrap().unwrap();
    assert_eq!((meta.size, meta.modified_ms), (3, Some(1_600_000_000_000)));
    assert_eq!(read(&target), "old+");
    RealDisk.replace(&staged, &target, &old).unwrap();
    assert_eq!(read(&target), "new");
    assert_eq!(RealDisk.list(&dir).unwrap(), ["t"]);
    RealDisk.rename_new(&target, &staged).unwrap();
    RealDisk.remove_file(&staged).unwrap();
    assert!(RealDisk.meta(&staged).unwrap().is_none());
    RealDisk
        .remove_dir_all(&root.path().join("d".repeat(60)))
        .unwrap();
}

/// Sets or clears the read-only attribute; cleared again so the temp folder can be removed.
#[allow(
    clippy::permissions_set_readonly_false,
    reason = "Windows: this clears only the read-only attribute"
)]
fn set_read_only(path: &Path, on: bool) {
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_readonly(on);
    std::fs::set_permissions(path, permissions).unwrap();
}

#[test]
fn the_read_only_attribute_reads_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let (locked, open) = (dir.path().join("ro"), dir.path().join("rw"));
    put(&locked, "x");
    put(&open, "x");
    set_read_only(&locked, true);
    let read_only = RealDisk.meta(&locked).unwrap().unwrap().read_only;
    set_read_only(&locked, false);
    assert!(read_only);
    assert!(!RealDisk.meta(&open).unwrap().unwrap().read_only);
}

#[test]
fn a_target_held_without_sharing_is_unchanged_with_error_32() {
    let dir = tempfile::tempdir().unwrap();
    let (staged, target, old) = (
        dir.path().join("s"),
        dir.path().join("t"),
        dir.path().join("o"),
    );
    put(&target, "old");
    put(&staged, "new");
    let holder = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&target)
        .unwrap();
    let result = RealDisk.replace(&staged, &target, &old);
    drop(holder);
    assert!(
        matches!(&result, Err(ReplaceError::Unchanged(e)) if e.raw_os_error() == Some(32)),
        "{result:?}"
    );
    assert_eq!((read(&target), read(&staged)), ("old".into(), "new".into()));
    assert!(RealDisk.meta(&old).unwrap().is_none());
}

#[test]
fn a_target_open_with_all_share_flags_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let (staged, target, old) = (
        dir.path().join("s"),
        dir.path().join("t"),
        dir.path().join("o"),
    );
    put(&target, "old");
    put(&staged, "new");
    let mut reader = File::open(&target).unwrap();
    RealDisk.replace(&staged, &target, &old).unwrap();
    let mut held = String::new();
    reader.read_to_string(&mut held).unwrap();
    drop(reader);
    assert_eq!(held, "old", "the open handle keeps the old file");
    assert_eq!(read(&target), "new");
    assert_eq!(RealDisk.list(dir.path()).unwrap(), ["t"]);
}

#[test]
fn rename_new_refuses_an_existing_name_and_keeps_both_files() {
    let dir = tempfile::tempdir().unwrap();
    let (from, to) = (dir.path().join("a"), dir.path().join("b"));
    put(&from, "a");
    put(&to, "b");
    assert_eq!(
        RealDisk.rename_new(&from, &to).unwrap_err().kind(),
        ErrorKind::AlreadyExists
    );
    assert_eq!((read(&from), read(&to)), ("a".into(), "b".into()));
}

#[test]
fn a_junction_or_a_relative_path_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (real, link) = (dir.path().join("real"), dir.path().join("link"));
    RealDisk.create_dir(&real).unwrap();
    let made = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&real)
        .output()
        .unwrap();
    assert!(made.status.success(), "{made:?}");
    let error = RealDisk.meta(&link).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    let error = RealDisk.meta(Path::new("relative.txt")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
}
