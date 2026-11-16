//! Keyboard wiring of the shell: its actions, key bindings, F6 pane cycling and hover-tip Escape.
//! A child module of `shell`, so it reads the shell's private fields.

use gpui_kit::{App, Context, KeyBinding, Window};

use super::Shell;
use crate::chrome::sidebar::{self, DismissTip, ToggleRail};
use crate::chrome::topbar_status;
use crate::layers::keys_sheet;
use crate::screens::compare_tab::OWN;
use crate::screens::{compare_tab_keys, folders_view, large_view};
use crate::start;

gpui_kit::actions!(shell, [NewTab, CloseTab, NextTab, PrevTab, CyclePane]);

/// Key context of the shell's root element; the tab keys are bound inside it.
pub(super) const CONTEXT: &str = "Shell";

/// Binds the shell, start screen, sidebar, and topbar keys; call once after `sidewise_kit::init`.
pub fn bind_keys(cx: &mut App) {
    start::bind_keys(cx);
    folders_view::bind_keys(cx);
    large_view::bind_keys(cx);
    compare_tab_keys::bind_keys(cx);
    sidebar::bind_keys(cx);
    topbar_status::bind_keys(cx);
    keys_sheet::bind_keys(cx);
    crate::menu_host::bind_menu_keys(cx);
    cx.bind_keys([
        KeyBinding::new("ctrl-t", NewTab, Some(CONTEXT)),
        KeyBinding::new("1", NewTab, Some(OWN)),
        KeyBinding::new("ctrl-w", CloseTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-tab", NextTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-shift-tab", PrevTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-\\", ToggleRail, Some(CONTEXT)),
        KeyBinding::new("f6", CyclePane, Some(CONTEXT)),
        KeyBinding::new("shift-f6", CyclePane, Some(CONTEXT)),
        KeyBinding::new("escape", DismissTip, Some(CONTEXT)),
        KeyBinding::new("alt-v", topbar_status::ViewMenu, Some(CONTEXT)),
    ]);
}

impl Shell {
    /// F6 / Shift+F6: moves focus to the other of the two panes, the sidebar or the active start
    /// screen. Does not resync tab data, so an overlay-only hover tip stays.
    pub(super) fn cycle_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar.read(cx).contains_focus(window, cx) {
            self.activated(window, cx);
        } else {
            self.sidebar
                .update(cx, |sidebar, cx| sidebar.focus_primary(window, cx));
            cx.notify();
        }
    }

    /// `Escape` from Main: dismisses a shown hover tip through the sidebar, else passes on.
    pub(super) fn dismiss_tip(&mut self, cx: &mut Context<Self>) {
        if !self
            .sidebar
            .update(cx, |sidebar, cx| sidebar.dismiss_tip(cx))
        {
            cx.propagate();
        }
    }
}
