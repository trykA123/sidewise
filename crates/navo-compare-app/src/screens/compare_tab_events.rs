//! The compare tab's event handling: Folders and compare-screen events decide which view shows
//! and which pair opens (H3).

use gpui_kit::Focusable as _;
use navo_ui::browser::FolderSession;
use navo_ui::compare_diff::PairRequest;
use navo_ui::prelude::*;
use navo_ui::window_view::ScreenEvent;

use super::compare_run::RunKind;
use super::compare_tab::{CompareRequest, CompareTab, TabView};
use super::folders_view::FoldersEvent;
use super::pair_requests::{neighbour, pair_request};

impl CompareTab {
    /// Subscribes to the Folders view and the compare screen; for two files, opens their pair
    /// at once. Needs the window, so the shell calls it right after the entity is made.
    pub fn attach(
        &mut self,
        request: &CompareRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let folders = cx.subscribe_in(&self.folders, window, |tab, _, event, window, cx| {
            tab.on_folders_event(event, window, cx);
        });
        let screen = cx.subscribe_in(&self.screen, window, |tab, _, event, window, cx| {
            tab.on_screen_event(event, window, cx);
        });
        let large = cx.subscribe_in(&self.large, window, |tab, _, event, window, cx| {
            tab.on_large_event(event, window, cx);
        });
        self.subscriptions.extend([folders, screen, large]);
        if self.run.kind() == RunKind::Files {
            let roots = self.run.roots();
            let path = request
                .left
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            let pair = PairRequest {
                id: 0,
                path,
                left: Some(roots[0].clone()),
                right: Some(roots[1].clone()),
            };
            self.show_pair(pair, window, cx);
        }
    }

    /// A Folders row opened: shows the pair of that row, when its row has arrived.
    fn on_folders_event(
        &mut self,
        event: &FoldersEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let FoldersEvent::OpenPair(id) = event;
        let asked = self.run.session().and_then(|session| session.pair_of(*id));
        if let Some(asked) = asked {
            self.show_pair(pair_request(asked), window, cx);
        }
    }

    /// The compare screen closed or asked to walk; settings and resync have no place in the alpha.
    fn on_screen_event(
        &mut self,
        event: &ScreenEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ScreenEvent::Closed { reveal } => {
                self.view = TabView::Folders;
                if let Some(path) = reveal {
                    self.folders
                        .update(cx, |view, cx| view.select_path(path, cx));
                }
                self.folders.read(cx).focus_handle(cx).focus(window, cx);
                cx.notify();
            }
            ScreenEvent::WalkFile { from, step } => {
                let next = self
                    .run
                    .session()
                    .and_then(|session: &FolderSession| neighbour(session, (*from, *step)));
                if let Some(pair) = next {
                    self.show_pair(pair, window, cx);
                }
            }
            ScreenEvent::OpenSettings(_) | ScreenEvent::Resync => {}
        }
    }

    /// Opens `request` in the file diff and shows it; the screen moves focus to the diff.
    fn show_pair(&mut self, request: PairRequest, window: &mut Window, cx: &mut Context<Self>) {
        let id = request.id;
        let opened = self.screen.update(cx, |screen, cx| {
            screen.show_pair((request, None), window, cx)
        });
        if opened {
            self.view = TabView::File;
            self.shown = Some(id);
            cx.notify();
        }
    }
}
