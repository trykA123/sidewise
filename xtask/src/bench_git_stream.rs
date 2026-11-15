//! Composes the deterministic fast-import stream for the Git bench fixture.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bench_git_commits::write_commits;
use crate::bench_git_fixture::{self, Layout, StreamError};
use crate::bench_git_payload::{BASE_BLOBS, LargeBlob, write_payload};

/// Writes the complete fixture stream for `layout`.
pub(crate) fn write_stream<W: Write>(
    writer: &mut W,
    layout: &Layout,
    cancel: &AtomicBool,
) -> Result<(), StreamError> {
    Stream {
        writer,
        layout,
        cancel,
    }
    .run()
}

struct Stream<'a, W> {
    writer: &'a mut W,
    layout: &'a Layout,
    cancel: &'a AtomicBool,
}

impl<W: Write> Stream<'_, W> {
    /// Writes the prefix, blobs, commit records, and trailing marker.
    fn run(mut self) -> Result<(), StreamError> {
        self.check()?;
        self.writer.write_all(b"feature done\n")?;
        let small = self.layout.small;
        let c = self.layout.counts;
        for index in 0..small {
            self.small(index + 1, bench_git_fixture::small_content(index))?;
        }
        for index in 0..c.modified {
            self.small(
                small + index + 1,
                bench_git_fixture::small_modified_content(index),
            )?;
        }
        for index in 0..c.added {
            self.small(
                small + c.modified + index + 1,
                bench_git_fixture::small_content(small + index),
            )?;
        }
        if self.layout.large {
            let b = small + c.modified + c.added;
            for (offset, blob) in BASE_BLOBS.iter().enumerate() {
                self.large(b + offset as u64 + 1, *blob)?;
            }
            self.large(b + 5, LargeBlob::Text20Changed)?;
        }
        write_commits(&mut *self.writer, self.layout, self.cancel)?;
        self.check()?;
        self.writer.write_all(b"done\n")?;
        Ok(())
    }

    /// Writes one 128-byte small blob.
    fn small(&mut self, mark: u64, payload: [u8; 128]) -> Result<(), StreamError> {
        self.check()?;
        self.header(mark, 128)?;
        self.check()?;
        self.writer.write_all(&payload)?;
        self.writer.write_all(b"\n")?;
        Ok(())
    }

    /// Writes one large payload blob with its exact length.
    fn large(&mut self, mark: u64, blob: LargeBlob) -> Result<(), StreamError> {
        self.check()?;
        self.header(mark, blob.bytes())?;
        write_payload(&mut *self.writer, blob, self.cancel)?;
        self.check()?;
        self.writer.write_all(b"\n")?;
        Ok(())
    }

    /// Writes the blob mark and counted payload header.
    fn header(&mut self, mark: u64, len: u64) -> Result<(), StreamError> {
        write!(self.writer, "blob\nmark :{mark}\ndata {len}\n")?;
        Ok(())
    }

    /// Returns `Cancelled` when the caller has set the flag.
    fn check(&self) -> Result<(), StreamError> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(StreamError::Cancelled)
        } else {
            Ok(())
        }
    }
}
