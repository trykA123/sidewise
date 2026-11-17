//! Types of the undo seam (NC148): how the app reports the end of an undo to the compare
//! screen. The hooks in `hooks.rs` move them between the UI and the app.

use std::sync::Arc;

use navo_core::Location;

use super::CopyResult;

/// The end of one undo: the files it touched and how it went. `Copied` means the copy was
/// undone; `Refused` carries the text for the status line and nothing was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoDone {
    pub targets: Vec<Location>,
    pub result: CopyResult,
}

/// Receives the end of an undo on any thread; it wakes the window.
pub type UndoSink = Arc<dyn Fn(UndoDone) + Send + Sync>;
