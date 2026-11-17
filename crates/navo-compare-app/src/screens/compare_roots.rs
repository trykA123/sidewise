//! Turns the two sides of a compare into a run kind and two roots; part of the compare run model.
//! A path side is a local root; a Git side is a stored-form `git` folder root.

use std::path::Path;

use navo_compare_cli::args::SideSpec;
use sidewise_core::{Location, ProviderPath};
use sidewise_git::location::{Form, encode};

use super::compare_run::{RunError, RunKind};
use crate::start::Side;

/// Classifies two roots by the file system: both folders, both files, or neither of those.
/// A missing side is `Missing`; another read failure is `Location`; mixed kinds are `Mixed`.
pub fn classify(left: &Path, right: &Path) -> Result<RunKind, RunError> {
    let left = std::fs::metadata(left).map_err(|error| root_error(&error, Side::Left))?;
    let right = std::fs::metadata(right).map_err(|error| root_error(&error, Side::Right))?;
    match (left.is_dir(), right.is_dir()) {
        (true, true) => Ok(RunKind::Folders),
        (false, false) => Ok(RunKind::Files),
        _ => Err(RunError::Mixed),
    }
}

/// Maps a missing root to `Missing`; any other metadata failure is a location error.
fn root_error(error: &std::io::Error, side: Side) -> RunError {
    if error.kind() == std::io::ErrorKind::NotFound {
        RunError::Missing(side)
    } else {
        RunError::Location(error.to_string())
    }
}

/// Turns a path into the local location the compare engine reads; keeps non-UTF-8 names on Unix.
#[cfg(unix)]
pub(crate) fn location_of(path: &Path) -> Result<Location, RunError> {
    use std::os::unix::ffi::OsStrExt as _;
    Location::local(ProviderPath::from_bytes(path.as_os_str().as_bytes()))
        .map_err(|error| RunError::Location(error.to_string()))
}

/// Turns a path into the local location the compare engine reads; keeps wide names on Windows.
#[cfg(windows)]
pub(crate) fn location_of(path: &Path) -> Result<Location, RunError> {
    use std::os::windows::ffi::OsStrExt as _;
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    Location::local(ProviderPath::from_wide(&wide))
        .map_err(|error| RunError::Location(error.to_string()))
}

/// Turns one side into its root: a local location for a path, a `git` location for a revision.
/// A Git side whose location cannot be encoded is a `Location` error.
fn root_of(spec: &SideSpec) -> Result<Location, RunError> {
    match spec {
        SideSpec::Path(path) => location_of(path),
        SideSpec::GitRev { repo, rev } => encode(repo, rev, Form::Stored)
            .map_err(|error| RunError::Location(format!("{error:?}"))),
    }
}

/// Classifies a Git side against a path side: a folder is `Folders`, a file is `Mixed`.
/// A missing or unreadable path is reported for the side it sits on.
fn git_with_path(path: &Path, side: Side) -> Result<RunKind, RunError> {
    let meta = std::fs::metadata(path).map_err(|error| root_error(&error, side))?;
    if meta.is_dir() {
        Ok(RunKind::Folders)
    } else {
        Err(RunError::Mixed)
    }
}

/// Decides the run kind and the two roots for a pair of sides.
/// Two paths classify by the file system; a Git side counts as a folder; errors name the side.
pub fn roots_of(left: &SideSpec, right: &SideSpec) -> Result<(RunKind, [Location; 2]), RunError> {
    let kind = match (left, right) {
        (SideSpec::Path(l), SideSpec::Path(r)) => classify(l, r)?,
        (SideSpec::GitRev { .. }, SideSpec::GitRev { .. }) => RunKind::Folders,
        (SideSpec::GitRev { .. }, SideSpec::Path(path)) => git_with_path(path, Side::Right)?,
        (SideSpec::Path(path), SideSpec::GitRev { .. }) => git_with_path(path, Side::Left)?,
    };
    Ok((kind, [root_of(left)?, root_of(right)?]))
}
