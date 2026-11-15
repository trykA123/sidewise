//! Bounded payload bytes for the large Git bench fixture blobs.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bench_git_fixture::StreamError;

const MIB: u64 = 1024 * 1024;
const CHUNK: usize = 64 * 1024;
const SEED: u64 = 0x5eed;

/// One large fixture payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LargeBlob {
    Text5,
    Text10,
    Text20,
    Binary45,
    Text20Changed,
}

impl LargeBlob {
    /// Returns the exact payload size in bytes.
    pub(crate) fn bytes(self) -> u64 {
        match self {
            Self::Text5 => 5 * MIB,
            Self::Text10 => 10 * MIB,
            Self::Text20 | Self::Text20Changed => 20 * MIB,
            Self::Binary45 => 45 * MIB,
        }
    }

    /// Returns the approved repository-relative path.
    pub(crate) fn path(self) -> &'static str {
        match self {
            Self::Text5 => "large/text_5mb.log",
            Self::Text10 => "large/text_10mb.log",
            Self::Text20 | Self::Text20Changed => "large/text_20mb.log",
            Self::Binary45 => "large/binary_45mb.bin",
        }
    }
}

/// The four original payloads in base commit order.
pub(crate) const BASE_BLOBS: [LargeBlob; 4] = [
    LargeBlob::Text5,
    LargeBlob::Text10,
    LargeBlob::Text20,
    LargeBlob::Binary45,
];

/// Writes only the body bytes of `blob`, checking cancellation per chunk.
pub(crate) fn write_payload<W: Write>(
    writer: &mut W,
    blob: LargeBlob,
    cancel: &AtomicBool,
) -> Result<(), StreamError> {
    match blob {
        LargeBlob::Binary45 => write_binary(writer, blob.bytes(), cancel),
        _ => write_text(
            writer,
            blob.bytes(),
            blob == LargeBlob::Text20Changed,
            cancel,
        ),
    }
}

/// Writes NUL bytes in chunks of at most 64 KiB.
fn write_binary<W: Write>(
    writer: &mut W,
    bytes: u64,
    cancel: &AtomicBool,
) -> Result<(), StreamError> {
    let buffer = [0_u8; CHUNK];
    let mut left = bytes;
    while left > 0 {
        check(cancel)?;
        let len = left.min(CHUNK as u64) as usize;
        writer.write_all(&buffer[..len])?;
        left -= len as u64;
    }
    Ok(())
}

/// Writes 64-byte seeded text lines in chunks of at most 64 KiB.
fn write_text<W: Write>(
    writer: &mut W,
    bytes: u64,
    changed: bool,
    cancel: &AtomicBool,
) -> Result<(), StreamError> {
    let mut buffer = [b' '; CHUNK];
    let mut line = 0_u64;
    let mut left = bytes;
    while left > 0 {
        check(cancel)?;
        let mut offset = 0;
        while offset < CHUNK {
            let text = format!("line {line:016x} seed {SEED:016x}");
            let start = offset;
            buffer[start..start + 64].fill(b' ');
            buffer[start..start + text.len()].copy_from_slice(text.as_bytes());
            buffer[start + 63] = b'\n';
            if changed && line == 0 {
                buffer[start] = b'L';
            }
            line += 1;
            offset += 64;
        }
        writer.write_all(&buffer)?;
        left -= CHUNK as u64;
    }
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
