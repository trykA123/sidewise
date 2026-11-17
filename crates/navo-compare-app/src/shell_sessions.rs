//! The session book wiring of the shell: records compares, reopens stored sessions, syncs the rows.
//! A child module of `shell_tabs`, so it reads the shell's private fields.

use std::path::Path;

use gpui_kit::Global;
use navo_compare_cli::args::SideSpec;
use navo_compare_profile::book::{SessionBook, Visit, now_ms};
use navo_compare_profile::recent::StartRows;
use sidewise_kit::prelude::*;

use super::{Shell, TabKind};
use crate::layers::toast;
use crate::screens::compare_tab::CompareRequest;
use crate::start_tabs::{StartChoice, StartMode};

/// The app's session book as a GPUI global; absent when the profile did not open and in tests that install none.
struct Sessions(SessionBook);
impl Global for Sessions {}

/// The stored rows, or no rows without a book.
pub(super) fn stored_rows(cx: &App) -> StartRows {
    cx.try_global::<Sessions>()
        .map(|sessions| sessions.0.rows().clone())
        .unwrap_or_default()
}

/// Unix seconds now.
fn now_s() -> i64 {
    i64::try_from(now_ms() / 1000).unwrap_or(i64::MAX)
}

impl Shell {
    /// Opens the session book (`explicit` folder, else the platform folders) as the app global; Err is the text to print.
    pub(crate) fn install_sessions(explicit: Option<&Path>, cx: &mut App) -> Result<(), String> {
        let book = SessionBook::open(explicit, now_s())?;
        cx.set_global(Sessions(book));
        Ok(())
    }

    /// Pushes the stored rows into the sidebar and into every start tab.
    pub(crate) fn sync_rows(&self, cx: &mut Context<Self>) {
        let rows = stored_rows(cx);
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.set_rows(rows.clone(), cx));
        let screens = self.tabs.iter().filter_map(|tab| match &tab.kind {
            TabKind::Start(screen) => Some(screen.clone()),
            TabKind::Compare(_) => None,
        });
        for screen in screens.collect::<Vec<_>>() {
            let rows = rows.clone();
            screen.update(cx, |screen, cx| {
                screen.rows = rows;
                screen.selected = None;
                cx.notify();
            });
        }
    }

    /// Deletes unnamed recent session `id`, refreshes the rows and toasts "Removed from Recent" with an Undo
    /// that saves it back. An error toasts its text and changes nothing.
    pub(in crate::shell) fn forget_row(&mut self, id: &str, cx: &mut Context<Self>) {
        if !cx.has_global::<Sessions>() {
            return;
        }
        let session = match cx.global_mut::<Sessions>().0.forget(id, now_s()) {
            Ok(session) => session,
            Err(text) => return self.toast(text, cx),
        };
        self.sync_rows(cx);
        let shell = cx.weak_entity();
        let undo = move |cx: &mut App| {
            let restored = cx.global_mut::<Sessions>().0.restore(&session, now_s());
            let _ = shell.update(cx, |shell, cx| match restored {
                Ok(()) => shell.sync_rows(cx),
                Err(text) => shell.toast(text, cx),
            });
        };
        toast::notify_undo("Removed from Recent", undo, cx);
    }

    /// `open_compare`; when it returns Ok, remembers the request. An error already toasted.
    pub(crate) fn open_recorded(
        &mut self,
        (index, request): (usize, CompareRequest),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .open_compare((index, request.clone()), window, cx)
            .is_ok()
        {
            self.remember(&request, cx);
        }
    }

    /// Records the compare in the book (no book: nothing); a save error shows as a toast.
    pub(super) fn remember(&mut self, request: &CompareRequest, cx: &mut Context<Self>) {
        if !cx.has_global::<Sessions>() {
            return;
        }
        let (SideSpec::Path(left), SideSpec::Path(right)) = (&request.left, &request.right) else {
            return;
        };
        let visit = Visit {
            left: left.clone(),
            right: right.clone(),
            respect_gitignore: request.choice.respect_gitignore,
        };
        if let Err(text) = cx.global_mut::<Sessions>().0.record(&visit, now_ms()) {
            self.toast(text, cx);
        }
        self.sync_rows(cx);
    }

    /// Opens stored session `id` in the tab at `index`; a bad session shows one toast and the rows refresh.
    pub(in crate::shell) fn reopen(
        &mut self,
        (index, id): (usize, &str),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !cx.has_global::<Sessions>() {
            return;
        }
        match cx.global::<Sessions>().0.load(id) {
            Ok(visit) => {
                let choice = StartChoice {
                    mode: StartMode::Folder,
                    respect_gitignore: visit.respect_gitignore,
                };
                let request = CompareRequest {
                    left: SideSpec::Path(visit.left),
                    right: SideSpec::Path(visit.right),
                    choice,
                };
                self.open_recorded((index, request), window, cx);
            }
            Err(text) => {
                self.toast(text, cx);
                let _ = cx.global_mut::<Sessions>().0.refresh(now_s());
                self.sync_rows(cx);
            }
        }
    }
}
