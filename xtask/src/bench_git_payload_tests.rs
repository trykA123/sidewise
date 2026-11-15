//! Tests for bounded large-payload bytes.

use std::io::{self, ErrorKind, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bench_git_fixture::StreamError;
use crate::bench_git_payload::{self, BASE_BLOBS, LargeBlob};

const CHUNK: usize = 64 * 1024;

/// Builds the expected 64-byte line at `index`.
fn expected_line(index: u64, blob: LargeBlob) -> [u8; 64] {
    let seed: u64 = 0x5eed;
    let text = format!("line {index:016x} seed {seed:016x}");
    let mut line = [b' '; 64];
    line[..text.len()].copy_from_slice(text.as_bytes());
    line[63] = b'\n';
    if blob == LargeBlob::Text20Changed && index == 0 {
        line[0] = b'L';
    }
    line
}

/// Builds an invalid-data error with `message`.
fn invalid(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}

/// Validates every byte without collecting a blob.
struct ValidatingWriter {
    blob: LargeBlob,
    total: u64,
    writes: usize,
    max_write: usize,
    line: [u8; 64],
    lines: u64,
    first_line: Option<[u8; 64]>,
    last_line: Option<[u8; 64]>,
}

impl ValidatingWriter {
    /// Builds a validator for `blob` with zero counters.
    fn new(blob: LargeBlob) -> Self {
        Self {
            blob,
            total: 0,
            writes: 0,
            max_write: 0,
            line: [b' '; 64],
            lines: 0,
            first_line: None,
            last_line: None,
        }
    }

    /// Validates one payload byte and advances the counters.
    fn consume(&mut self, value: u8) -> io::Result<()> {
        let position = self.total;
        if self.blob == LargeBlob::Binary45 {
            if value != 0 {
                return Err(invalid("binary payload is not NUL"));
            }
            self.total += 1;
            return Ok(());
        }
        let column = (position % 64) as usize;
        if column == 0 {
            self.line = expected_line(position / 64, self.blob);
            if self.first_line.is_none() {
                self.first_line = Some(self.line);
            }
        }
        if value != self.line[column] {
            return Err(invalid("text payload differs from the seeded line"));
        }
        if column == 63 {
            self.last_line = Some(self.line);
            self.lines += 1;
        }
        self.total += 1;
        Ok(())
    }
}

impl Write for ValidatingWriter {
    /// Validates one chunk against the expected payload.
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.total + buffer.len() as u64 > self.blob.bytes() {
            return Err(invalid("payload longer than declared"));
        }
        self.writes += 1;
        self.max_write = self.max_write.max(buffer.len());
        for &value in buffer {
            self.consume(value)?;
        }
        Ok(buffer.len())
    }

    /// Flushes nothing because the validator has no buffer.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Writes and validates all bytes of `blob`.
fn validate(blob: LargeBlob) -> ValidatingWriter {
    let mut writer = ValidatingWriter::new(blob);
    bench_git_payload::write_payload(&mut writer, blob, &AtomicBool::new(false)).unwrap();
    writer
}

#[test]
fn paths_sizes_and_base_order_are_exact() {
    assert_eq!(
        BASE_BLOBS,
        [
            LargeBlob::Text5,
            LargeBlob::Text10,
            LargeBlob::Text20,
            LargeBlob::Binary45
        ]
    );
    assert_eq!(LargeBlob::Text5.bytes(), 5 * 1024 * 1024);
    assert_eq!(LargeBlob::Text10.bytes(), 10 * 1024 * 1024);
    assert_eq!(LargeBlob::Text20.bytes(), 20 * 1024 * 1024);
    assert_eq!(LargeBlob::Binary45.bytes(), 45 * 1024 * 1024);
    assert_eq!(LargeBlob::Text5.path(), "large/text_5mb.log");
    assert_eq!(LargeBlob::Text10.path(), "large/text_10mb.log");
    assert_eq!(LargeBlob::Text20.path(), "large/text_20mb.log");
    assert_eq!(LargeBlob::Binary45.path(), "large/binary_45mb.bin");
    assert_eq!(LargeBlob::Text20Changed.path(), LargeBlob::Text20.path());
    assert_eq!(LargeBlob::Text20Changed.bytes(), LargeBlob::Text20.bytes());
}

#[test]
fn every_payload_has_exact_length_and_bounded_chunks() {
    for blob in [
        LargeBlob::Text5,
        LargeBlob::Text10,
        LargeBlob::Text20,
        LargeBlob::Binary45,
        LargeBlob::Text20Changed,
    ] {
        let writer = validate(blob);
        assert_eq!(writer.total, blob.bytes());
        assert!(writer.writes > 0);
        assert!(writer.max_write <= CHUNK);
        assert_eq!(writer.writes as u64, blob.bytes() / CHUNK as u64);
        if blob == LargeBlob::Binary45 {
            assert!(writer.first_line.is_none());
            assert!(writer.last_line.is_none());
        } else {
            assert_eq!(writer.lines, blob.bytes() / 64);
            assert_eq!(writer.first_line, Some(expected_line(0, blob)));
            assert_eq!(
                writer.last_line,
                Some(expected_line(blob.bytes() / 64 - 1, blob))
            );
        }
    }
}

#[test]
fn text20_changed_differs_only_at_byte_zero() {
    let base = validate(LargeBlob::Text20);
    let changed = validate(LargeBlob::Text20Changed);
    let base_first = base.first_line.unwrap();
    let changed_first = changed.first_line.unwrap();

    assert_eq!(base.total, changed.total);
    assert_eq!(base.lines, changed.lines);
    assert_eq!(base_first[0], b'l');
    assert_eq!(changed_first[0], b'L');
    assert_eq!(&base_first[1..], &changed_first[1..]);
    assert_eq!(base.last_line, changed.last_line);
}

#[test]
fn pre_set_cancel_writes_nothing() {
    let cancel = AtomicBool::new(true);
    let mut writer = CountingWriter::default();
    let error =
        bench_git_payload::write_payload(&mut writer, LargeBlob::Text5, &cancel).unwrap_err();

    assert!(matches!(error, StreamError::Cancelled));
    assert_eq!(writer.total, 0);
}

#[test]
fn mid_body_cancel_stops_after_one_chunk() {
    let cancel = AtomicBool::new(false);
    let mut writer = CancelWriter::new(&cancel);
    let error =
        bench_git_payload::write_payload(&mut writer, LargeBlob::Text20, &cancel).unwrap_err();

    assert!(matches!(error, StreamError::Cancelled));
    assert_eq!(writer.total, CHUNK as u64);
    assert_eq!(writer.writes, 1);
}

#[test]
fn writer_error_is_propagated() {
    let mut writer = FailingWriter;
    let error =
        bench_git_payload::write_payload(&mut writer, LargeBlob::Text5, &AtomicBool::new(false))
            .unwrap_err();

    match error {
        StreamError::Io(error) => {
            assert_eq!(error.kind(), ErrorKind::Other);
            assert!(error.to_string().contains("payload boom"));
        }
        other => panic!("expected StreamError::Io, got {other:?}"),
    }
}

/// Counts written bytes without retaining them.
#[derive(Default)]
struct CountingWriter {
    total: u64,
}

impl Write for CountingWriter {
    /// Adds the chunk length to the byte count.
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.total += buffer.len() as u64;
        Ok(buffer.len())
    }

    /// Flushes nothing because the writer has no buffer.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Sets cancellation after its first write.
struct CancelWriter<'a> {
    cancel: &'a AtomicBool,
    total: u64,
    writes: usize,
}

impl<'a> CancelWriter<'a> {
    /// Builds a writer that cancels after one chunk.
    fn new(cancel: &'a AtomicBool) -> Self {
        Self {
            cancel,
            total: 0,
            writes: 0,
        }
    }
}

impl Write for CancelWriter<'_> {
    /// Counts a chunk and sets the cancel flag.
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.total += buffer.len() as u64;
        self.writes += 1;
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
        Err(io::Error::other("payload boom"))
    }

    /// Flushes nothing because the writer always fails.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
