//! Tests for deterministic fixture layout and small payloads.

use std::collections::HashSet;
use std::io;

use crate::bench_git_fixture::{self, StreamError};

/// Exact printable alphabet required after the index prefix.
const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 _-.";

#[test]
fn counts_at_1000() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    assert_eq!(layout.files, 1_000);
    assert!(!layout.large);
    assert_eq!(layout.small, 1_000);
    assert_eq!(layout.counts.modified, 70);
    assert_eq!(layout.counts.deleted, 10);
    assert_eq!(layout.counts.renamed, 5);
    assert_eq!(layout.counts.added, 15);
    assert_eq!(layout.counts.head, 1_005);
}

#[test]
fn default_large_counts() {
    let layout = bench_git_fixture::layout(100_004, true).unwrap();
    assert!(layout.large);
    assert_eq!(layout.small, 100_000);
    assert_eq!(layout.counts.modified, 7_000);
    assert_eq!(layout.counts.deleted, 1_000);
    assert_eq!(layout.counts.renamed, 500);
    assert_eq!(layout.counts.added, 1_500);
    assert_eq!(layout.counts.head, 100_504);
}

#[test]
fn boundary_layouts_are_validated_without_generation() {
    assert!(matches!(
        bench_git_fixture::layout(0, false),
        Err(StreamError::InvalidInput(_))
    ));

    let one = bench_git_fixture::layout(1, false).unwrap();
    assert_eq!(one.small, 1);
    assert_eq!(one.counts.head, 1);

    assert!(matches!(
        bench_git_fixture::layout(4, true),
        Err(StreamError::InvalidInput(_))
    ));

    let five = bench_git_fixture::layout(5, true).unwrap();
    assert!(five.large);
    assert_eq!(five.small, 1);
    assert_eq!(five.counts.modified, 0);
    assert_eq!(five.counts.deleted, 0);
    assert_eq!(five.counts.renamed, 0);
    assert_eq!(five.counts.added, 0);
    assert_eq!(five.counts.head, 5);

    let maximum = bench_git_fixture::layout(1_000_000, false).unwrap();
    assert_eq!(maximum.small, 1_000_000);
    assert_eq!(maximum.counts.modified, 70_000);
    assert_eq!(maximum.counts.deleted, 10_000);
    assert_eq!(maximum.counts.renamed, 5_000);
    assert_eq!(maximum.counts.added, 15_000);
    assert_eq!(maximum.counts.head, 1_005_000);

    assert!(matches!(
        bench_git_fixture::layout(1_000_001, false),
        Err(StreamError::InvalidInput(_))
    ));
}

#[test]
fn operation_ranges_are_contiguous_and_disjoint() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let modified_end = layout.counts.modified;
    let deleted_end = modified_end + layout.counts.deleted;
    let renamed_end = deleted_end + layout.counts.renamed;

    assert_eq!(modified_end, 70);
    assert_eq!(deleted_end, 80);
    assert_eq!(renamed_end, 85);
    assert!(renamed_end <= layout.small);

    let additions = layout.small..layout.small + layout.counts.added;
    assert_eq!(additions.start, 1_000);
    assert_eq!(additions.end, 1_015);
    assert!(additions.start >= renamed_end);
}

#[test]
fn small_path_geometry_is_exact() {
    assert_eq!(
        bench_git_fixture::small_path(0),
        "pkg_00/mod_00/file_00000.rs"
    );
    assert_eq!(
        bench_git_fixture::small_path(999),
        "pkg_09/mod_99/file_00000.rs"
    );
    assert_eq!(
        bench_git_fixture::small_path(1_000),
        "pkg_10/mod_00/file_00000.rs"
    );
    assert_eq!(
        bench_git_fixture::small_path(2_000),
        "pkg_00/mod_00/file_00001.rs"
    );
    assert_eq!(
        bench_git_fixture::small_path(999_999),
        "pkg_19/mod_99/file_00499.rs"
    );
}

#[test]
fn renamed_paths_keep_the_parents() {
    let base = bench_git_fixture::small_path(70);
    let renamed = bench_git_fixture::small_renamed_path(70);

    assert_eq!(renamed, "pkg_00/mod_70/file_00000.renamed.rs");
    assert_eq!(
        renamed.rsplit_once('/').unwrap().0,
        base.rsplit_once('/').unwrap().0
    );
    assert_ne!(renamed, base);
    assert_ne!(renamed, bench_git_fixture::small_path(1_000));
}

#[test]
fn content_is_128_ascii_bytes_with_unique_prefixes() {
    let mut prefixes = HashSet::new();

    for index in 0..1_000 {
        let content = bench_git_fixture::small_content(index);
        assert_eq!(content.len(), 128);
        assert_eq!(&content[..8], format!("{index:08x}").as_bytes());
        assert!(content[8..].iter().all(|byte| ALPHABET.contains(byte)));

        let prefix: [u8; 8] = content[..8].try_into().unwrap();
        assert!(prefixes.insert(prefix));
    }

    assert_eq!(prefixes.len(), 1_000);
}

#[test]
fn content_is_seeded_per_index() {
    assert_eq!(
        bench_git_fixture::small_content(42),
        bench_git_fixture::small_content(42)
    );
    assert_ne!(
        bench_git_fixture::small_content(42),
        bench_git_fixture::small_content(43)
    );
}

#[test]
fn modified_content_changes_only_byte_8() {
    let mut saw_a = false;
    let mut saw_other = false;

    for index in 0..2_000 {
        let base = bench_git_fixture::small_content(index);
        let modified = bench_git_fixture::small_modified_content(index);

        for (offset, (base_byte, modified_byte)) in base.iter().zip(modified.iter()).enumerate() {
            if offset == 8 {
                assert_ne!(base_byte, modified_byte);
            } else {
                assert_eq!(base_byte, modified_byte);
            }
        }

        let expected = if base[8] == b'a' { b'b' } else { b'a' };
        assert_eq!(modified[8], expected);
        if base[8] == b'a' {
            saw_a = true;
        } else {
            saw_other = true;
        }
    }

    assert!(saw_a);
    assert!(saw_other);
}

#[test]
fn stream_error_display_and_conversion() {
    let io_error = StreamError::from(io::Error::other("boom"));
    assert!(io_error.to_string().contains("boom"));
    assert_eq!(
        StreamError::InvalidInput("bad input").to_string(),
        "bad input"
    );
    assert_eq!(StreamError::Cancelled.to_string(), "stream cancelled");
}
