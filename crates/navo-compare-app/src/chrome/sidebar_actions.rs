//! Interactive sidebar row controls: shared button wrapper and primary control.

use gpui_kit::base::{ObservedElement, TestSupportExt as _};
use gpui_kit::{
    AnyElement, FocusHandle, KeyDownEvent, MouseButton, ReadGlobal as _, Role, Stateful,
    transparent_black,
};
use navo_ui::prelude::*;

use super::{Entry, Sidebar, SidebarEvent};
use crate::theme::SidewiseTheme;
use crate::theme_data::palettes::{DIMS, ModeTokens};

/// Specification for one interactive sidebar row control.
struct RowControl<'a> {
    id: ElementId,
    entry: &'a Entry,
    focus: FocusHandle,
    event: SidebarEvent,
}

impl Sidebar {
    /// Interactive `New comparison` row wrapping [`Self::action_row`].
    pub(super) fn primary_row(
        &self,
        entry: &Entry,
        m: &ModeTokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.action_row(
            RowControl {
                id: "new-compare".into(),
                entry,
                focus: self.new_compare_focus.clone(),
                event: SidebarEvent::NewCompare,
            },
            m,
            cx,
        )
        .into_any_element()
    }

    /// Builds an interactive button row around [`Self::row_view`] with focus tracking and activation.
    fn action_row(
        &self,
        control: RowControl<'_>,
        m: &ModeTokens,
        cx: &mut Context<Self>,
    ) -> ObservedElement<Stateful<Div>> {
        let RowControl {
            id,
            entry,
            focus,
            event,
        } = control;
        let (ring, hover) = (m.accent, m.accent_tint);
        let ring_w = px(SidewiseTheme::global(cx).spec.ring_px);
        self.row_view(entry, m, cx)
            .id(id)
            .role(Role::Button)
            .aria_label(entry.label.clone())
            .test_support()
            .track_focus(&focus)
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
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.stop_propagation();
                cx.emit(event);
            }))
            .on_key_down(|key: &KeyDownEvent, _, cx| {
                let stroke = &key.keystroke;
                if !stroke.modifiers.modified() && matches!(stroke.key.as_str(), "enter" | "space")
                {
                    cx.stop_propagation();
                }
            })
    }
}
