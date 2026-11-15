//! Tests for deterministic base and head commit records.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bench_git_commits;
use crate::bench_git_fixture::{self, Layout, StreamError};

/// Writes and returns all commit records for `layout`.
fn records(layout: &Layout) -> String {
    let mut output = Vec::new();
    bench_git_commits::write_commits(&mut output, layout, &AtomicBool::new(false)).unwrap();
    String::from_utf8(output).unwrap()
}

/// Splits `text` into base and head record slices.
fn split(text: &str) -> (&str, &str) {
    let base = text.find("commit refs/heads/base\n").unwrap();
    let head = text.find("commit refs/heads/head\n").unwrap();
    (&text[base..head], &text[head..])
}

#[test]
fn base_1000_records_have_exact_marks_and_header() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let text = records(&layout);
    let (base, _) = split(&text);

    assert!(!text.contains("feature done"));
    assert!(!text.contains("blob\n"));
    assert!(!text.contains("done\n"));
    assert!(base.starts_with("commit refs/heads/base\nmark :1086\n"));
    assert!(base.contains("author admin <admin@localhost> 1700000000 +0000\n"));
    assert!(base.contains("committer admin <admin@localhost> 1700000000 +0000\n"));
    assert!(base.contains("data 13\nbase fixture\n\n"));

    let base_marks: Vec<&str> = base
        .lines()
        .filter(|line| line.starts_with("M 100644 "))
        .collect();
    assert_eq!(base_marks.len(), 1_000);
    assert_eq!(base_marks[0], "M 100644 :1 pkg_00/mod_00/file_00000.rs");
    assert_eq!(
        base_marks[999],
        "M 100644 :1000 pkg_09/mod_99/file_00000.rs"
    );
}

#[test]
fn head_1000_header_and_parent_are_exact() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let text = records(&layout);
    let (_, head) = split(&text);

    assert!(head.starts_with("commit refs/heads/head\nmark :1087\n"));
    assert!(head.contains("author admin <admin@localhost> 1700000001 +0000\n"));
    assert!(head.contains("committer admin <admin@localhost> 1700000001 +0000\n"));
    assert!(head.contains("data 13\nhead fixture\n\nfrom refs/heads/base\n"));
    assert!(head.find("from refs/heads/base\n").unwrap() < head.find("D ").unwrap());
}

#[test]
fn head_1000_ranges_and_marks_are_exact() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let text = records(&layout);
    let (_, head) = split(&text);
    let head_lines: Vec<&str> = head.lines().collect();

    let deleted: Vec<&str> = head_lines
        .iter()
        .filter(|line| line.starts_with("D "))
        .copied()
        .collect();
    assert_eq!(deleted.len(), 10);
    assert_eq!(deleted[0], "D pkg_00/mod_70/file_00000.rs");
    assert_eq!(deleted[9], "D pkg_00/mod_79/file_00000.rs");

    let renamed: Vec<&str> = head_lines
        .iter()
        .filter(|line| line.starts_with("R "))
        .copied()
        .collect();
    assert_eq!(renamed.len(), 5);
    assert_eq!(
        renamed[0],
        "R pkg_00/mod_80/file_00000.rs pkg_00/mod_80/file_00000.renamed.rs"
    );
    assert_eq!(
        renamed[4],
        "R pkg_00/mod_84/file_00000.rs pkg_00/mod_84/file_00000.renamed.rs"
    );

    let modified_and_added: Vec<&str> = head_lines
        .iter()
        .filter(|line| line.starts_with("M 100644 "))
        .copied()
        .collect();
    assert_eq!(modified_and_added.len(), 85);
    assert_eq!(
        modified_and_added[0],
        "M 100644 :1001 pkg_00/mod_00/file_00000.rs"
    );
    assert_eq!(
        modified_and_added[69],
        "M 100644 :1070 pkg_00/mod_69/file_00000.rs"
    );
    assert_eq!(
        modified_and_added[70],
        "M 100644 :1071 pkg_10/mod_00/file_00000.rs"
    );
    assert_eq!(
        modified_and_added[84],
        "M 100644 :1085 pkg_10/mod_14/file_00000.rs"
    );
}

#[test]
fn large_mapping_uses_payload_paths_and_marks() {
    let layout = bench_git_fixture::layout(5, true).unwrap();
    let text = records(&layout);
    let (base, head) = split(&text);

    assert!(base.starts_with("commit refs/heads/base\nmark :7\n"));
    let base_marks: Vec<&str> = base
        .lines()
        .filter(|line| line.starts_with("M 100644 "))
        .collect();
    assert_eq!(base_marks.len(), 5);
    assert_eq!(base_marks[0], "M 100644 :1 pkg_00/mod_00/file_00000.rs");
    assert_eq!(base_marks[1], "M 100644 :2 large/text_5mb.log");
    assert_eq!(base_marks[2], "M 100644 :3 large/text_10mb.log");
    assert_eq!(base_marks[3], "M 100644 :4 large/text_20mb.log");
    assert_eq!(base_marks[4], "M 100644 :5 large/binary_45mb.bin");

    assert!(head.starts_with("commit refs/heads/head\nmark :8\n"));
    assert!(head.contains("from refs/heads/base\n"));
    let head_marks: Vec<&str> = head
        .lines()
        .filter(|line| line.starts_with("M 100644 "))
        .collect();
    assert_eq!(head_marks, ["M 100644 :6 large/text_20mb.log"]);
    assert!(!head.lines().any(|line| line.starts_with("D ")));
    assert!(!head.lines().any(|line| line.starts_with("R ")));
}

#[test]
fn pre_set_cancel_writes_nothing() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let cancel = AtomicBool::new(true);
    let mut output = Vec::new();
    let error = bench_git_commits::write_commits(&mut output, &layout, &cancel).unwrap_err();

    assert!(matches!(error, StreamError::Cancelled));
    assert!(output.is_empty());
}

#[test]
fn mid_record_cancel_stops_after_one_base_record() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let full = records(&layout);
    let first = full.find("M 100644 ").unwrap();
    let limit = first + full[first..].find('\n').unwrap() + 1;
    let cancel = AtomicBool::new(false);
    let mut writer = CancelAfter::new(&cancel, limit);
    let error = bench_git_commits::write_commits(&mut writer, &layout, &cancel).unwrap_err();

    assert!(matches!(error, StreamError::Cancelled));
    let text = String::from_utf8(writer.output).unwrap();
    assert!(text.contains("commit refs/heads/base"));
    assert!(!text.contains("commit refs/heads/head"));
    assert_eq!(text.matches("M 100644 ").count(), 1);
}

#[test]
fn writer_error_is_propagated() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let mut writer = FailingWriter;
    let error = bench_git_commits::write_commits(&mut writer, &layout, &AtomicBool::new(false))
        .unwrap_err();

    match error {
        StreamError::Io(error) => {
            assert_eq!(error.kind(), io::ErrorKind::Other);
            assert!(error.to_string().contains("commit boom"));
        }
        other => panic!("expected StreamError::Io, got {other:?}"),
    }
}

/// Cancels after `limit` bytes have been written.
struct CancelAfter<'a> {
    cancel: &'a AtomicBool,
    limit: u64,
    total: u64,
    output: Vec<u8>,
}

impl<'a> CancelAfter<'a> {
    /// Builds a writer with a byte limit.
    fn new(cancel: &'a AtomicBool, limit: usize) -> Self {
        Self {
            cancel,
            limit: limit as u64,
            total: 0,
            output: Vec::new(),
        }
    }
}

impl Write for CancelAfter<'_> {
    /// Appends a chunk and sets cancel at the limit.
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.output.extend_from_slice(buffer);
        self.total += buffer.len() as u64;
        if self.total >= self.limit {
            self.cancel.store(true, Ordering::Relaxed);
        }
        Ok(buffer.len())
    }

    /// Flushes nothing because the writer has no buffer.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Fails every write to exercise error propagation.
struct FailingWriter;

impl Write for FailingWriter {
    /// Returns a synthetic I/O error.
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("commit boom"))
    }

    /// Flushes nothing because the writer always fails.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
