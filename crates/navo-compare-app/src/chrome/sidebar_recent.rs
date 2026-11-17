//! The sidebar's Recent entries (B5): a right click asks the shell for the start row menu.
//! A child of `sidebar_actions`, so it reads the sidebar's private parts.

use gpui_kit::base::TestSupportExt as _;
use gpui_kit::{AnyElement, ClickEvent};
use sidewise_kit::prelude::*;

use super::super::{Entry, Sidebar};
use super::SidebarEvent;
use crate::theme_data::palettes::ModeTokens;

impl Sidebar {
    /// Recent entry `index`: the static row, whose right click emits `RecentMenu(index, at)`.
    pub(in super::super) fn recent_row(
        &self,
        (index, entry): (usize, &Entry),
        m: &ModeTokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.row_view(entry, m, cx)
            .id(("Recent", index))
            .aria_label(entry.label.clone())
            .test_support()
            .on_aux_click(cx.listener(move |_, click: &ClickEvent, _, cx| {
                if click.is_right_click() {
                    cx.stop_propagation();
                    cx.emit(SidebarEvent::RecentMenu(index, click.position()));
                }
            }))
            .into_any_element()
    }
}
