//! Temp-folder tests of the Unix `RealDisk` contract.

use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::{Disk, RealDisk, ReplaceError};

fn put(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
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

#[test]
fn replace_renames_staged_over_the_target_and_leaves_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let (staged, target, old) = (
        dir.path().join("s"),
        dir.path().join("t"),
        dir.path().join("o"),
    );
    put(&target, "old");
    put(&staged, "new");
    let staged_id = RealDisk.meta(&staged).unwrap().unwrap().identity;
    RealDisk.replace(&staged, &target, &old).unwrap();
    assert_eq!(read(&target), "new");
    assert_eq!(
        RealDisk.meta(&target).unwrap().unwrap().identity,
        staged_id,
        "a rename, not a copy"
    );
    assert_eq!(RealDisk.list(dir.path()).unwrap(), ["t"]);
}

#[test]
fn a_failed_replace_is_unchanged_and_keeps_the_target() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("t");
    put(&target, "old");
    let error = RealDisk.replace(&dir.path().join("gone"), &target, &dir.path().join("o"));
    assert!(matches!(error, Err(ReplaceError::Unchanged(e)) if e.kind() == ErrorKind::NotFound));
    assert_eq!(read(&target), "old");
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
    RealDisk.remove_file(&to).unwrap();
    RealDisk.rename_new(&from, &to).unwrap();
    assert_eq!(RealDisk.list(dir.path()).unwrap(), ["b"]);
}

#[test]
fn create_new_refuses_an_existing_name_and_appends_reopen_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let mut file = RealDisk.create_new(&path).unwrap();
    file.write_all(b"one\n").unwrap();
    file.sync().unwrap();
    assert_eq!(
        RealDisk.create_new(&path).err().unwrap().kind(),
        ErrorKind::AlreadyExists
    );
    RealDisk
        .open_append(&path)
        .unwrap()
        .write_all(b"two\n")
        .unwrap();
    assert_eq!(read(&path), "one\ntwo\n");
    let missing = RealDisk
        .open_append(&dir.path().join("none"))
        .err()
        .unwrap();
    assert_eq!(missing.kind(), ErrorKind::NotFound, "append never creates");
}

#[test]
fn meta_reads_size_time_and_identity_and_none_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    assert_eq!(RealDisk.meta(&path).unwrap(), None);
    put(&path, "12345");
    RealDisk.set_modified(&path, -1_500).unwrap();
    let meta = RealDisk.meta(&path).unwrap().unwrap();
    assert_eq!(
        (meta.size, meta.modified_ms, meta.read_only),
        (5, Some(-1_500), false)
    );
    assert!(meta.identity.is_some());
}

#[test]
fn a_mode_0444_file_reads_read_only_and_copy_mode_carries_it() {
    let dir = tempfile::tempdir().unwrap();
    let (locked, copy) = (dir.path().join("l"), dir.path().join("c"));
    put(&locked, "x");
    put(&copy, "y");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o444)).unwrap();
    assert!(RealDisk.meta(&locked).unwrap().unwrap().read_only);
    RealDisk.copy_mode(&locked, &copy).unwrap();
    let mode = std::fs::metadata(&copy).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o444);
}

#[test]
fn meta_refuses_a_symlink_even_to_a_real_file() {
    let dir = tempfile::tempdir().unwrap();
    let (file, link) = (dir.path().join("f"), dir.path().join("l"));
    put(&file, "x");
    std::os::unix::fs::symlink(&file, &link).unwrap();
    assert_eq!(
        RealDisk.meta(&link).unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
}

#[test]
fn folders_are_made_listed_sorted_synced_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let job = dir.path().join("job");
    RealDisk.create_dir(&job).unwrap();
    for name in ["b", "a", "c"] {
        put(&job.join(name), name);
    }
    RealDisk.sync_dir(&job).unwrap();
    assert_eq!(RealDisk.list(&job).unwrap(), ["a", "b", "c"]);
    RealDisk.remove_dir_all(&job).unwrap();
    assert_eq!(RealDisk.meta(&job).unwrap(), None);
}
