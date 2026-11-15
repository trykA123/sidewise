//! Interactive sidebar row controls: shared button/tab wrapper, primary row, and tab controls.

use gpui_kit::assets::IconName;
use gpui_kit::base::{ObservedElement, TestSupportExt as _};
use gpui_kit::{
    AnyElement, FocusHandle, KeyDownEvent, MouseButton, ReadGlobal as _, Role, Stateful,
    transparent_black,
};
use navo_ui::prelude::*;
use navo_ui::primitives::icon;

use super::{Entry, Sidebar, SidebarEvent};
use crate::theme::SidewiseTheme;
use crate::theme_data::palettes::{DIMS, ModeTokens};

/// Direction for keyboard row traversal inside [`Sidebar`].
#[derive(Clone, Copy)]
pub(super) enum RowDirection {
    Next,
    Previous,
}

/// Descriptors for rendering an open-tab row from `Sidebar::tabs`.
pub(super) struct TabRow<'a> {
    index: usize,
    entry: &'a Entry,
}

impl<'a> TabRow<'a> {
    /// Creates a tab row descriptor for `index` and `entry`.
    pub(super) fn new(index: usize, entry: &'a Entry) -> Self {
        Self { index, entry }
    }
}

/// Specification for one interactive sidebar row control.
struct RowControl<'a> {
    id: ElementId,
    entry: &'a Entry,
    role: Role,
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
                role: Role::Button,
                focus: self.new_compare_focus.clone(),
                event: SidebarEvent::NewCompare,
            },
            m,
            cx,
        )
        .into_any_element()
    }

    /// Interactive open-tab row with `Role::Tab`, selection state, left accent bar, and close child.
    pub(super) fn tab_row(
        &self,
        tab: TabRow<'_>,
        m: &ModeTokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = tab.index == self.active;
        let ring_w = px(SidewiseTheme::global(cx).spec.ring_px);
        self.action_row(
            RowControl {
                id: ("open-tab", tab.index).into(),
                entry: tab.entry,
                role: Role::Tab,
                focus: self.tab_focuses[tab.index].clone(),
                event: SidebarEvent::SelectTab(tab.index),
            },
            m,
            cx,
        )
        .aria_selected(selected)
        .relative()
        .when(selected, |row| {
            row.bg(m.selected).child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(1.))
                    .bottom(px(1.))
                    .w(ring_w)
                    .bg(m.accent),
            )
        })
        .when(!self.rail, |row| {
            row.child(self.tab_close(tab.index, m, cx))
        })
        .into_any_element()
    }

    /// Non-focusable tab close button (hidden in rail) emitting `CloseTab` only.
    fn tab_close(&self, index: usize, m: &ModeTokens, cx: &mut Context<Self>) -> AnyElement {
        let hover = m.accent_tint;
        div()
            .id(("tab-close", index))
            .role(Role::Button)
            .aria_label("Close tab")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(DIMS.row_h)
            .rounded(DIMS.btn_r)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.stop_propagation();
                cx.emit(SidebarEvent::CloseTab(index));
            }))
            .child(icon(IconName::Close, DIMS.fs, m.muted))
            .into_any_element()
    }

    /// Builds an interactive row around [`Self::row_view`] with role, focus tracking, and activation.
    fn action_row(
        &self,
        control: RowControl<'_>,
        m: &ModeTokens,
        cx: &mut Context<Self>,
    ) -> ObservedElement<Stateful<Div>> {
        let RowControl {
            id,
            entry,
            role,
            focus,
            event,
        } = control;
        let (ring, hover) = (m.accent, m.accent_tint);
        let ring_w = px(SidewiseTheme::global(cx).spec.ring_px);
        self.row_view(entry, m, cx)
            .id(id)
            .role(role)
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

    /// Moves focus across `New comparison` and `Open tabs`, wrapping in both directions.
    pub(super) fn focus_row(&self, direction: RowDirection, window: &mut Window, cx: &mut App) {
        let count = 1 + self.tab_focuses.len();
        let current = std::iter::once(&self.new_compare_focus)
            .chain(self.tab_focuses.iter())
            .position(|handle| handle.is_focused(window));
        let target_ix = match (current, direction) {
            (Some(i), RowDirection::Next) => (i + 1) % count,
            (Some(i), RowDirection::Previous) => (i + count - 1) % count,
            (None, RowDirection::Next) => 0,
            (None, RowDirection::Previous) => count - 1,
        };
        let target = if target_ix == 0 {
            &self.new_compare_focus
        } else {
            &self.tab_focuses[target_ix - 1]
        };
        target.focus(window, cx);
    }
}
