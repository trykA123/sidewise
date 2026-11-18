//! Rewrites the hash cache file from the live rows: temp file, sync, then rename.
//! Only the writer calls it, at open, when the file is damaged.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use super::file::{Row, with_suffix};
use super::record::{Digest, MAGIC, encode};

/// Replaces the file at `path` with `MAGIC` plus every row in write order.
/// Fails on any I/O error; the temp file is removed then and `path` is left as it was.
pub(super) fn rewrite(path: &Path, rows: &HashMap<Digest, Row>) -> io::Result<()> {
    let temp = with_suffix(path, ".tmp");
    let result = write_rows(&temp, rows).and_then(|()| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Writes `MAGIC` and the rows sorted by `seq` to a new file at `temp`, then syncs it.
fn write_rows(temp: &Path, rows: &HashMap<Digest, Row>) -> io::Result<()> {
    let mut ordered: Vec<_> = rows.iter().collect();
    ordered.sort_unstable_by_key(|(_, row)| row.seq);
    let mut out = BufWriter::new(File::create(temp)?);
    out.write_all(&MAGIC)?;
    for (key, row) in ordered {
        out.write_all(&encode(key, &row.hash))?;
    }
    out.into_inner()
        .map_err(io::IntoInnerError::into_error)?
        .sync_all()
}
