//! The compare screen's local file reads and the no-op hash cache.

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

use sidewise_core::Location;
use sidewise_core::paging::ByteSource;
use sidewise_fs::PagedFile;
use sidewise_ui::window_view::CompareFiles;

/// A hash cache that keeps nothing: every lookup misses, so content checks hash afresh.
pub struct NoHashes;

impl sidewise_engine::model::HashCache for NoHashes {
    /// Always misses, so a content check hashes the file again.
    fn get(&self, _key: &sidewise_engine::model::ContentKey) -> Option<[u8; 32]> {
        None
    }

    /// Drops the hash: this cache keeps nothing.
    fn put(&self, _key: &sidewise_engine::model::ContentKey, _hash: [u8; 32]) {}
}

/// The compare screen's file reads over the local disk: whole bytes up to a limit, and paged sources.
/// Read-only by construction (CompareFiles has no write).
pub fn compare_files() -> CompareFiles {
    CompareFiles {
        bytes: Arc::new(read_bytes),
        source: Arc::new(open_source),
    }
}

/// Reads at most `limit` bytes of a local file; fails on a non-local location, an open/read error,
/// or a file over `limit` bytes ("Too large to open here (over N MB).").
fn read_bytes(location: &Location, limit: u64) -> Result<Vec<u8>, String> {
    let path = path_of(location)?;
    let file = File::open(&path).map_err(|error| format!("Cannot open it: {error}"))?;
    let size = file
        .metadata()
        .map_err(|error| format!("Cannot open it: {error}"))?
        .len();
    if size > limit {
        return Err(too_large(limit));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read it: {error}"))?;
    if bytes.len() as u64 > limit {
        return Err(too_large(limit));
    }
    Ok(bytes)
}

/// Opens a local file as a paged byte source (sidewise_fs::PagedFile).
fn open_source(location: &Location) -> Result<Arc<dyn ByteSource + Send + Sync>, String> {
    let path = path_of(location)?;
    let paged = PagedFile::open(&path)?;
    Ok(Arc::new(paged))
}

/// The local path of a location; fails with "Only local files open here." when it is not local.
fn path_of(location: &Location) -> Result<PathBuf, String> {
    if !location.is_local() {
        return Err("Only local files open here.".to_owned());
    }
    sidewise_fs::os_path(location).map_err(|error| error.message())
}

/// The "too large" message for `limit` bytes, in whole megabytes.
fn too_large(limit: u64) -> String {
    format!(
        "Too large to open here (over {} MB).",
        limit / (1024 * 1024)
    )
}
