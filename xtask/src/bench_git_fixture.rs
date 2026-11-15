//! Deterministic layout and small-file bytes for the Git bench fixture, test-only until final CLI activation.

use std::fmt;
use std::io;

use crate::bench_rng::{Rng, splitmix64};

const MAX_FILES: u64 = 1_000_000;
const SEED: u64 = 0x5eed;
const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 _-.";
/// Counts of one fixture layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counts {
    pub modified: u64,
    pub deleted: u64,
    pub renamed: u64,
    pub added: u64,
    pub head: u64,
}

/// Validated counts and geometry inputs for one fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub files: u64,
    pub large: bool,
    pub small: u64,
    pub counts: Counts,
}

/// What layout or fixture byte generation can fail on.
#[derive(Debug)]
pub enum StreamError {
    Io(io::Error),
    InvalidInput(&'static str),
    Cancelled,
}
impl fmt::Display for StreamError {
    /// Writes the user-facing error text.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "stream I/O error: {error}"),
            Self::InvalidInput(message) => formatter.write_str(message),
            Self::Cancelled => formatter.write_str("stream cancelled"),
        }
    }
}

impl From<io::Error> for StreamError {
    /// Wraps an I/O error without changing it.
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Validates the base count and returns the fixture layout.
pub fn layout(files: u64, large: bool) -> Result<Layout, StreamError> {
    if files == 0 || files > MAX_FILES || (large && files < 5) {
        return Err(StreamError::InvalidInput("invalid file count"));
    }
    let large_count = if large { 4 } else { 0 };
    let small = files
        .checked_sub(large_count)
        .ok_or(StreamError::InvalidInput("small count underflow"))?;
    let modified = small
        .checked_mul(7)
        .ok_or(StreamError::InvalidInput("modified count overflow"))?
        / 100;
    let deleted = small / 100;
    let renamed = small / 200;
    let added = small
        .checked_mul(3)
        .ok_or(StreamError::InvalidInput("added count overflow"))?
        / 200;
    let head = files
        .checked_sub(deleted)
        .and_then(|value| value.checked_add(added))
        .ok_or(StreamError::InvalidInput("head count overflow"))?;
    small
        .checked_add(added)
        .ok_or(StreamError::InvalidInput("addition range overflow"))?;
    let blob_count = small
        .checked_add(modified)
        .and_then(|value| value.checked_add(added))
        .and_then(|value| value.checked_add(if large { 5 } else { 0 }))
        .ok_or(StreamError::InvalidInput("blob mark overflow"))?;
    blob_count
        .checked_add(2)
        .ok_or(StreamError::InvalidInput("commit mark overflow"))?;
    let counts = Counts {
        modified,
        deleted,
        renamed,
        added,
        head,
    };
    Ok(Layout {
        files,
        large,
        small,
        counts,
    })
}

/// Builds the shared path prefix and adds `suffix`.
fn path(index: u64, suffix: &str) -> String {
    format!(
        "pkg_{:02}/mod_{:02}/file_{:05}{suffix}",
        (index / 100) % 20,
        index % 100,
        index / 2_000
    )
}

/// Returns the repository-relative path of small file `index`.
pub fn small_path(index: u64) -> String {
    path(index, ".rs")
}

/// Returns the rename destination of small file `index`.
pub(crate) fn small_renamed_path(index: u64) -> String {
    path(index, ".renamed.rs")
}

/// Builds the 128-byte payload of small file `index`.
pub fn small_content(index: u64) -> [u8; 128] {
    let mut content = [0_u8; 128];
    let prefix = format!("{index:08x}");
    content[..8].copy_from_slice(&prefix.as_bytes()[..8]);
    let mut rng = Rng::new(SEED ^ splitmix64(index));
    for byte in &mut content[8..] {
        *byte = ALPHABET[(rng.next_u64() % ALPHABET.len() as u64) as usize];
    }
    content
}

/// Returns the modified payload of small file `index`, changing only byte 8.
pub(crate) fn small_modified_content(index: u64) -> [u8; 128] {
    let mut content = small_content(index);
    content[8] = if content[8] == b'a' { b'b' } else { b'a' };
    content
}
