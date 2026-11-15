//! Deterministic base and head commit records for the Git bench fixture.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bench_git_fixture::{self, Layout, StreamError};
use crate::bench_git_payload::{BASE_BLOBS, LargeBlob};

const IDENTITY: &str = "admin <admin@localhost>";
const BASE_MESSAGE: &[u8] = b"base fixture\n";
const HEAD_MESSAGE: &[u8] = b"head fixture\n";

#[derive(Clone, Copy)]
enum CommitKind {
    Base,
    Head,
}

impl CommitKind {
    /// Returns the ref, timestamp, and counted message for this kind.
    fn fields(self) -> (&'static str, u64, &'static [u8]) {
        match self {
            Self::Base => ("refs/heads/base", 1_700_000_000, BASE_MESSAGE),
            Self::Head => ("refs/heads/head", 1_700_000_001, HEAD_MESSAGE),
        }
    }
}

/// Writes base then head commit records for a validated layout.
pub(crate) fn write_commits<W: Write>(
    writer: &mut W,
    layout: &Layout,
    cancel: &AtomicBool,
) -> Result<(), StreamError> {
    base_records(writer, layout, cancel)?;
    head_records(writer, layout, cancel)?;
    Ok(())
}

/// Returns B plus the five large marks when `large`.
fn blob_count(layout: &Layout) -> u64 {
    let c = layout.counts;
    layout.small + c.modified + c.added + if layout.large { 5 } else { 0 }
}

/// Writes the base commit and its file records.
fn base_records<W: Write>(
    writer: &mut W,
    layout: &Layout,
    cancel: &AtomicBool,
) -> Result<(), StreamError> {
    let small = layout.small;
    let b = blob_count(layout) - if layout.large { 5 } else { 0 };
    check(cancel)?;
    header(writer, CommitKind::Base, blob_count(layout) + 1)?;
    for index in 0..small {
        check(cancel)?;
        let path = bench_git_fixture::small_path(index);
        writeln!(writer, "M 100644 :{} {path}", index + 1)?;
    }
    if layout.large {
        for (offset, blob) in BASE_BLOBS.iter().enumerate() {
            check(cancel)?;
            writeln!(
                writer,
                "M 100644 :{} {}",
                b + offset as u64 + 1,
                blob.path()
            )?;
        }
    }
    Ok(())
}

/// Writes the head commit, parent, and changed file records.
fn head_records<W: Write>(
    writer: &mut W,
    layout: &Layout,
    cancel: &AtomicBool,
) -> Result<(), StreamError> {
    let c = layout.counts;
    let small = layout.small;
    let b = blob_count(layout) - if layout.large { 5 } else { 0 };
    check(cancel)?;
    header(writer, CommitKind::Head, blob_count(layout) + 2)?;
    check(cancel)?;
    writer.write_all(b"from refs/heads/base\n")?;

    let m = c.modified;
    for index in m..m + c.deleted {
        check(cancel)?;
        writeln!(writer, "D {}", bench_git_fixture::small_path(index))?;
    }
    for index in m + c.deleted..m + c.deleted + c.renamed {
        check(cancel)?;
        let old = bench_git_fixture::small_path(index);
        let new = bench_git_fixture::small_renamed_path(index);
        writeln!(writer, "R {old} {new}")?;
    }
    for index in 0..m {
        check(cancel)?;
        let path = bench_git_fixture::small_path(index);
        writeln!(writer, "M 100644 :{} {path}", small + index + 1)?;
    }
    for index in 0..c.added {
        check(cancel)?;
        let path = bench_git_fixture::small_path(small + index);
        writeln!(writer, "M 100644 :{} {path}", small + m + index + 1)?;
    }
    if layout.large {
        check(cancel)?;
        let path = LargeBlob::Text20Changed.path();
        writeln!(writer, "M 100644 :{} {path}", b + 5)?;
    }
    Ok(())
}

/// Writes the commit header selected by `kind`.
fn header<W: Write>(writer: &mut W, kind: CommitKind, mark: u64) -> Result<(), StreamError> {
    let (reference, time, message) = kind.fields();
    write!(
        writer,
        "commit {reference}\nmark :{mark}\nauthor {IDENTITY} {time} +0000\ncommitter {IDENTITY} {time} +0000\ndata 13\n"
    )?;
    writer.write_all(message)?;
    writer.write_all(b"\n")?;
    Ok(())
}

/// Returns `Cancelled` when the caller has set the flag.
fn check(cancel: &AtomicBool) -> Result<(), StreamError> {
    if cancel.load(Ordering::Relaxed) {
        Err(StreamError::Cancelled)
    } else {
        Ok(())
    }
}
