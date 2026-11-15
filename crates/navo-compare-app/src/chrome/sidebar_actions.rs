//! Primary sidebar control (`New comparison`): interactive button semantics, focus and activation.

use gpui_kit::base::TestSupportExt as _;
use gpui_kit::{AnyElement, KeyDownEvent, MouseButton, ReadGlobal as _, Role, transparent_black};
use navo_ui::prelude::*;

use super::{Entry, Sidebar, SidebarEvent};
use crate::theme::SidewiseTheme;
use crate::theme_data::palettes::{DIMS, ModeTokens};

impl Sidebar {
    /// Interactive `New comparison` row wrapping [`Self::row_view`] with button role and focus.
    pub(super) fn primary_row(
        &self,
        entry: &Entry,
        m: &ModeTokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (ring, hover) = (m.accent, m.accent_tint);
        let ring_w = px(SidewiseTheme::global(cx).spec.ring_px);
        let focus = self.new_compare_focus.clone();
        self.row_view(entry, m, cx)
            .id("new-compare")
            .role(Role::Button)
            .aria_label("New comparison")
            .test_support()
            .track_focus(&self.new_compare_focus)
            .rounded(DIMS.btn_r)
            .border(ring_w)
            .border_color(transparent_black())
            .when(!self.rail, |row| row.px_1())
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .focus_visible(move |style| style.border(ring_w).border_color(ring))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                focus.focus(window, cx);
            })
            .on_click(cx.listener(|_, _, _, cx| {
                cx.stop_propagation();
                cx.emit(SidebarEvent::NewCompare);
            }))
            .on_key_down(|key: &KeyDownEvent, _, cx| {
                let stroke = &key.keystroke;
                if !stroke.modifiers.modified() && matches!(stroke.key.as_str(), "enter" | "space")
                {
                    cx.stop_propagation();
                }
            })
            .into_any_element()
    }
}
