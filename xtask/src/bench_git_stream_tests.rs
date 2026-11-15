//! Tests for the composed deterministic fast-import stream.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bench_git_fixture::{self, Layout, StreamError};
use crate::bench_git_stream;

const MIB: u64 = 1024 * 1024;

/// Distinguishes blob bodies from commit messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Blob,
    Commit,
}

/// One counted data block in the stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    kind: Kind,
    mark: u64,
    len: u64,
}

/// Parser state: command line, body, or separator.
enum Mode {
    Line,
    Body {
        remaining: u64,
        kind: Kind,
        mark: u64,
        len: u64,
    },
    Separator {
        kind: Kind,
        mark: u64,
        len: u64,
    },
}

/// Parses command lines and counts bodies without retaining them.
struct Probe {
    mode: Mode,
    line: Vec<u8>,
    pending: Option<Kind>,
    mark: u64,
    entries: Vec<Entry>,
    blobs: usize,
    feature: bool,
    base: bool,
    head: bool,
    parent: bool,
    done: bool,
    body_bytes: u64,
}

impl Probe {
    /// Starts a probe at a command line.
    fn new() -> Self {
        Self {
            mode: Mode::Line,
            line: Vec::new(),
            pending: None,
            mark: 0,
            entries: Vec::new(),
            blobs: 0,
            feature: false,
            base: false,
            head: false,
            parent: false,
            done: false,
            body_bytes: 0,
        }
    }

    /// Advances the probe by one byte.
    fn consume(&mut self, byte: u8) -> io::Result<()> {
        match self.mode {
            Mode::Line => {
                self.line.push(byte);
                if byte == b'\n' {
                    self.line_done()?;
                }
            }
            Mode::Body {
                remaining,
                kind,
                mark,
                len,
            } => {
                self.body_bytes += 1;
                let next = remaining - 1;
                self.mode = if next == 0 {
                    Mode::Separator { kind, mark, len }
                } else {
                    Mode::Body {
                        remaining: next,
                        kind,
                        mark,
                        len,
                    }
                };
            }
            Mode::Separator { kind, mark, len } => {
                if byte != b'\n' {
                    return Err(invalid("missing data separator"));
                }
                self.entries.push(Entry { kind, mark, len });
                self.mode = Mode::Line;
            }
        }
        Ok(())
    }

    /// Handles one complete command line.
    fn line_done(&mut self) -> io::Result<()> {
        let line = std::mem::take(&mut self.line);
        let line = line.strip_suffix(b"\n").unwrap_or(&line);
        if line == b"feature done" {
            self.feature = true;
        } else if line == b"done" {
            self.done = true;
        } else if line == b"blob" {
            self.pending = Some(Kind::Blob);
            self.blobs += 1;
        } else if line.starts_with(b"commit refs/heads/base") {
            self.pending = Some(Kind::Commit);
            self.base = true;
        } else if line.starts_with(b"commit refs/heads/head") {
            self.pending = Some(Kind::Commit);
            self.head = true;
        } else if line.starts_with(b"from refs/heads/base") {
            self.parent = true;
        } else if let Some(value) = line.strip_prefix(b"mark :") {
            self.mark = parse_number(value)?;
        } else if let Some(value) = line.strip_prefix(b"data ") {
            let kind = self.pending.ok_or_else(|| invalid("data before header"))?;
            let len = parse_number(value)?;
            self.mode = Mode::Body {
                remaining: len,
                kind,
                mark: self.mark,
                len,
            };
        }
        Ok(())
    }
}

impl Write for Probe {
    /// Feeds one chunk to the probe.
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        for &byte in buffer {
            self.consume(byte)?;
        }
        Ok(buffer.len())
    }

    /// Flushes nothing because the probe has no buffer.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Parses one decimal field.
fn parse_number(bytes: &[u8]) -> io::Result<u64> {
    std::str::from_utf8(bytes)
        .map_err(|_| invalid("non-UTF-8 number"))?
        .parse()
        .map_err(|_| invalid("invalid number"))
}

/// Builds an invalid-data error.
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Writes `layout` into a fresh probe.
fn probe(layout: &Layout) -> Probe {
    let mut probe = Probe::new();
    bench_git_stream::write_stream(&mut probe, layout, &AtomicBool::new(false)).unwrap();
    assert!(matches!(probe.mode, Mode::Line));
    assert!(probe.line.is_empty());
    probe
}

/// Writes `layout` into a byte vector for small-body checks.
fn stream_bytes(layout: &Layout) -> Vec<u8> {
    let mut output = Vec::new();
    bench_git_stream::write_stream(&mut output, layout, &AtomicBool::new(false)).unwrap();
    output
}

/// Returns the body of small blob `mark` and checks its separator.
fn body_for(stream: &[u8], mark: u64, len: usize) -> &[u8] {
    let header = format!("blob\nmark :{mark}\ndata {len}\n");
    let at = stream
        .windows(header.len())
        .position(|window| window == header.as_bytes())
        .unwrap();
    let start = at + header.len();
    assert_eq!(stream[start + len], b'\n');
    &stream[start..start + len]
}

#[test]
fn stream_1000_has_exact_blob_marks_and_commit_order() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let probe = probe(&layout);

    assert!(probe.feature);
    assert!(probe.base);
    assert!(probe.head);
    assert!(probe.parent);
    assert!(probe.done);
    assert_eq!(probe.blobs, 1_085);
    assert_eq!(probe.entries.len(), 1_087);

    for (offset, entry) in probe.entries[..1_085].iter().enumerate() {
        assert_eq!(
            *entry,
            Entry {
                kind: Kind::Blob,
                mark: offset as u64 + 1,
                len: 128
            }
        );
    }
    assert_eq!(
        probe.entries[1_085],
        Entry {
            kind: Kind::Commit,
            mark: 1_086,
            len: 13
        }
    );
    assert_eq!(
        probe.entries[1_086],
        Entry {
            kind: Kind::Commit,
            mark: 1_087,
            len: 13
        }
    );
}

#[test]
fn stream_1000_reuses_small_payload_helpers() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let stream = stream_bytes(&layout);

    assert_eq!(
        body_for(&stream, 1, 128),
        &bench_git_fixture::small_content(0)
    );
    assert_eq!(
        body_for(&stream, 1_001, 128),
        &bench_git_fixture::small_modified_content(0)
    );
    assert_eq!(
        body_for(&stream, 1_071, 128),
        &bench_git_fixture::small_content(1_000)
    );
}

#[test]
fn large_stream_has_exact_lengths_and_order() {
    let layout = bench_git_fixture::layout(5, true).unwrap();
    let probe = probe(&layout);
    let lengths = [128, 5 * MIB, 10 * MIB, 20 * MIB, 45 * MIB, 20 * MIB];

    assert!(probe.feature);
    assert!(probe.base);
    assert!(probe.head);
    assert!(probe.parent);
    assert!(probe.done);
    assert_eq!(probe.blobs, 6);
    assert_eq!(probe.entries.len(), 8);

    for (offset, length) in lengths.iter().enumerate() {
        assert_eq!(
            probe.entries[offset],
            Entry {
                kind: Kind::Blob,
                mark: offset as u64 + 1,
                len: *length
            }
        );
    }
    assert_eq!(
        probe.entries[6],
        Entry {
            kind: Kind::Commit,
            mark: 7,
            len: 13
        }
    );
    assert_eq!(
        probe.entries[7],
        Entry {
            kind: Kind::Commit,
            mark: 8,
            len: 13
        }
    );
    let bodies: u64 = lengths.iter().sum::<u64>() + 26;
    assert_eq!(probe.body_bytes, bodies);
}

#[test]
fn pre_set_cancel_writes_nothing() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let cancel = AtomicBool::new(true);
    let mut output = Vec::new();
    let error = bench_git_stream::write_stream(&mut output, &layout, &cancel).unwrap_err();

    assert!(matches!(error, StreamError::Cancelled));
    assert!(output.is_empty());
}

#[test]
fn mid_stream_cancel_stops_after_feature_header() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let cancel = AtomicBool::new(false);
    let mut writer = CancelAfterFirstWrite::new(&cancel);
    let error = bench_git_stream::write_stream(&mut writer, &layout, &cancel).unwrap_err();

    assert!(matches!(error, StreamError::Cancelled));
    assert_eq!(writer.output, b"feature done\n");
}

#[test]
fn writer_error_is_propagated() {
    let layout = bench_git_fixture::layout(1_000, false).unwrap();
    let mut writer = FailingWriter;
    let error =
        bench_git_stream::write_stream(&mut writer, &layout, &AtomicBool::new(false)).unwrap_err();

    match error {
        StreamError::Io(error) => {
            assert_eq!(error.kind(), io::ErrorKind::Other);
            assert!(error.to_string().contains("stream boom"));
        }
        other => panic!("expected StreamError::Io, got {other:?}"),
    }
}

/// Cancels after its first write.
struct CancelAfterFirstWrite<'a> {
    cancel: &'a AtomicBool,
    output: Vec<u8>,
}

impl<'a> CancelAfterFirstWrite<'a> {
    /// Builds a writer that cancels on the first chunk.
    fn new(cancel: &'a AtomicBool) -> Self {
        Self {
            cancel,
            output: Vec::new(),
        }
    }
}

impl Write for CancelAfterFirstWrite<'_> {
    /// Stores a chunk and sets the cancel flag.
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.output.extend_from_slice(buffer);
        self.cancel.store(true, Ordering::Relaxed);
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
        Err(io::Error::other("stream boom"))
    }

    /// Flushes nothing because the writer always fails.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
