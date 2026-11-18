//! Tests for the file-backed hash cache: open, lock, replay and the open-time rewrite.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use navo_compare::model::{ContentKey, HashCache};
use navo_core::{FileIdentity, FileIdentityKind, ProviderId, ProviderPath};

use super::file::{CacheMode, FileHashes};
use super::record::{MAGIC, encode, key_digest};

/// A local key for `name`, modified an hour ago, with a native identity.
fn key(name: &str) -> ContentKey {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    ContentKey {
        provider: ProviderId::local(),
        full_path: ProviderPath::from_text(&format!("/data/{name}")),
        size: 10,
        modified: Some(i64::try_from(now).unwrap() - 3_600_000),
        file_id: Some(FileIdentity {
            volume: 7,
            file: [name.len() as u8; 16],
            kind: FileIdentityKind::Native,
        }),
    }
}

/// The cache path under `sub`, whose folder does not exist yet.
fn cache_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("sub").join("hashes.v1")
}

/// One encoded record for `key` and `hash`.
fn row(key: &ContentKey, hash: [u8; 32]) -> Vec<u8> {
    encode(&key_digest(key).unwrap(), &hash).to_vec()
}

/// Writes `bytes` to `path`, creating its folder.
fn write_file(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// True when the `.tmp` sibling of `path` exists.
fn temp_exists(path: &Path) -> bool {
    let mut name = path.as_os_str().to_os_string();
    name.push(".tmp");
    PathBuf::from(name).exists()
}

#[test]
fn hashes_survive_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);
    let (a, b) = (key("a"), key("bb"));
    let bytes = [
        MAGIC.to_vec(),
        row(&a, [1; 32]),
        row(&b, [2; 32]),
        row(&a, [3; 32]),
    ]
    .concat();
    write_file(&path, &bytes);
    for _ in 0..2 {
        let store = FileHashes::open(&path);
        assert_eq!(store.mode(), CacheMode::Writer);
        assert_eq!(store.get(&a), Some([3; 32]), "later records win");
        assert_eq!(store.get(&b), Some([2; 32]));
        assert_eq!(store.get(&key("ccc")), None);
        assert_eq!(store.len(), 2);
    }
    assert_eq!(
        fs::read(&path).unwrap(),
        bytes,
        "a clean file is not rewritten"
    );
}

#[test]
fn a_second_store_on_the_same_file_only_reads() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);
    let a = key("a");
    write_file(&path, &[MAGIC.to_vec(), row(&a, [1; 32])].concat());
    let first = FileHashes::open(&path);
    assert_eq!(first.mode(), CacheMode::Writer);
    let torn = [MAGIC.to_vec(), row(&a, [1; 32]), vec![9; 10]].concat();
    fs::write(&path, &torn).unwrap();
    let second = FileHashes::open(&path);
    assert_eq!(second.mode(), CacheMode::Reader);
    assert_eq!(second.get(&a), Some([1; 32]));
    assert_eq!(fs::read(&path).unwrap(), torn, "a reader never rewrites");
    drop(second);
    drop(first);
    let third = FileHashes::open(&path);
    assert_eq!(third.mode(), CacheMode::Writer, "the lock is freed on drop");
}

#[test]
fn a_torn_tail_is_rewritten_at_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);
    let (a, b) = (key("a"), key("bb"));
    let mut bad = row(&b, [2; 32]);
    bad[40] ^= 1;
    write_file(
        &path,
        &[MAGIC.to_vec(), row(&a, [1; 32]), bad, vec![9; 10]].concat(),
    );
    let store = FileHashes::open(&path);
    assert_eq!(store.mode(), CacheMode::Writer);
    assert_eq!(store.get(&a), Some([1; 32]));
    assert_eq!(store.get(&b), None);
    assert_eq!(
        fs::read(&path).unwrap(),
        [MAGIC.to_vec(), row(&a, [1; 32])].concat()
    );
    assert!(!temp_exists(&path));
}

#[test]
fn a_bad_header_starts_empty_and_rewrites() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);
    let a = key("a");
    write_file(
        &path,
        &[b"SWHASH\x00\x02".to_vec(), row(&a, [1; 32])].concat(),
    );
    let store = FileHashes::open(&path);
    assert!(store.is_empty());
    assert_eq!(store.get(&a), None);
    assert_eq!(fs::read(&path).unwrap(), MAGIC.to_vec());
}

#[test]
fn a_failed_rewrite_removes_the_temp_and_stops_writing() {
    let dir = tempfile::tempdir().unwrap();
    let path = cache_path(&dir);
    fs::create_dir_all(&path).unwrap();
    let store = FileHashes::open(&path);
    assert_eq!(
        store.mode(),
        CacheMode::Reader,
        "a failed rewrite drops the writer"
    );
    assert!(store.is_empty());
    assert!(path.is_dir());
    assert!(!temp_exists(&path));
}

#[test]
fn memory_mode_keeps_no_file() {
    let store = FileHashes::memory();
    assert_eq!(store.mode(), CacheMode::Memory);
    assert!(store.is_empty());
    assert_eq!(store.get(&key("a")), None);
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("plain-file");
    fs::write(&blocker, b"x").unwrap();
    let path = blocker.join("hashes.v1");
    let store = FileHashes::open(&path);
    assert_eq!(
        store.mode(),
        CacheMode::Memory,
        "no parent folder means memory mode"
    );
    assert!(!path.exists());
}
