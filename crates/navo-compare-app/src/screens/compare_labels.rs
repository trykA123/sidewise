//! Labels of a compare tab's sides: the path or repository, the header root and the tab title.
//! The compare tab builds its Folders header and its title from these.

use std::path::Path;

use navo_compare_cli::args::SideSpec;
use sidewise_kit::prelude::*;

use super::compare_tab::CompareRequest;
use super::folders_head::SideRoot;

/// The path of a `Path` side, or the repository of a `GitRev` side.
pub(super) fn path_of(spec: &SideSpec) -> &Path {
    match spec {
        SideSpec::Path(path) => path,
        SideSpec::GitRev { repo, .. } => repo,
    }
}

/// The header's view of one side: its path, plus the revision for a Git side.
pub(super) fn side_root(spec: &SideSpec) -> SideRoot {
    SideRoot {
        path: path_of(spec).to_path_buf(),
        rev: match spec {
            SideSpec::Path(_) => None,
            SideSpec::GitRev { rev, .. } => Some(rev.clone()),
        },
    }
}

/// `"<left name> ↔ <right name>"` from the last path segment of each side; a Git side is
/// `name@rev`.
pub(super) fn title_of(request: &CompareRequest) -> SharedString {
    let name = |spec: &SideSpec| {
        let path = path_of(spec);
        let base = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        match spec {
            SideSpec::Path(_) => base,
            SideSpec::GitRev { rev, .. } => format!("{base}@{rev}"),
        }
    };
    format!("{} ↔ {}", name(&request.left), name(&request.right)).into()
}
