//! One command-line side: an existing path, else `repo@rev` from the right, else a path.
//! Split from `args.rs` so a later atom can grow the side parser without nearing the file cap.

use std::path::Path;

use crate::args::{ArgError, SideSpec};

/// Longest accepted Git revision, in bytes (NC21).
const MAX_REV_BYTES: usize = 256;

/// Parses one side: an existing whole path, else `repo@rev` from the right, else a path.
/// Fails with `BadRev` when a selected revision breaks NC21.
pub fn side(text: &str, cwd: &Path, exists: &dyn Fn(&Path) -> bool) -> Result<SideSpec, ArgError> {
    let text = crate::quote::unquote(text);
    let whole = cwd.join(text);
    if exists(&whole) {
        return Ok(SideSpec::Path(whole));
    }
    for (at, _) in text.rmatch_indices('@') {
        let repo_text = &text[..at];
        if repo_text.is_empty() {
            continue;
        }
        let repo = cwd.join(repo_text);
        if exists(&repo) {
            let rev = &text[at + 1..];
            check_rev(rev)?;
            return Ok(SideSpec::GitRev {
                repo,
                rev: rev.to_owned(),
            });
        }
    }
    Ok(SideSpec::Path(whole))
}

/// Accepts a revision unless it is empty, dash-led, control-bearing or over 256 bytes.
fn check_rev(rev: &str) -> Result<(), ArgError> {
    let bad = rev.is_empty()
        || rev.starts_with('-')
        || rev.chars().any(char::is_control)
        || rev.len() > MAX_REV_BYTES;
    if bad {
        return Err(ArgError::BadRev(rev.to_owned()));
    }
    Ok(())
}
