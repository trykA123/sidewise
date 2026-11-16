//! The sidebar tab context menu (CM6b): Close tab and Close other tabs, shown through the shared
//! menu host. A child module of `shell`, so it reads the shell's private fields.

use gpui_kit::{Context, Pixels, Point, Window};

use super::Shell;
use super::shell_keys::CloseTab;
use crate::menu_host::{MenuAt, MenuLine, show_menu};

gpui_kit::actions!(shell, [CloseOthers]);

/// The two menu lines; Close other tabs is dimmed when `tabs` is one.
fn tab_lines(tabs: usize) -> [MenuLine; 2] {
    [
        MenuLine::Run {
            label: "Close tab".into(),
            action: Box::new(CloseTab),
            enabled: true,
        },
        MenuLine::Run {
            label: "Close other tabs".into(),
            action: Box::new(CloseOthers),
            enabled: tabs > 1,
        },
    ]
}

impl Shell {
    /// Closes every tab but the active one, last first, each through `close_tab`.
    pub(super) fn close_others(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for index in (0..self.tabs.len()).rev() {
            if index != self.active {
                self.close_tab(index, window, cx);
            }
        }
    }

    /// Opens the tab menu at `anchor`; the chosen action runs on the focus held now (the tab was
    /// just selected). With nothing focused no menu opens.
    pub(super) fn show_tab_menu(
        &mut self,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(focus) = window.focused(cx) else {
            return;
        };
        let at = MenuAt {
            anchor,
            focus,
            title: "Tab",
        };
        let lines = tab_lines(self.tabs.len());
        show_menu(self, (&lines, at, |shell| &mut shell.menu), window, cx);
    }
}

#[cfg(test)]
#[path = "shell_menu_tests.rs"]
mod menu_tests;
