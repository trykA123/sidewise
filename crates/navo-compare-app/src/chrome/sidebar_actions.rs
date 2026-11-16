//! Interactive sidebar row controls: shared button/tab wrapper, primary row, and tab controls.

use gpui_kit::assets::IconName;
use gpui_kit::base::{ObservedElement, TestSupportExt as _};
use gpui_kit::{
    AnyElement, Bounds, FocusHandle, HoverListenerMode, KeyDownEvent, MouseButton, ReadGlobal as _,
    Role, Stateful, canvas, transparent_black,
};
use navo_compare_profile::recent::StartRows;
use navo_ui::prelude::*;
use navo_ui::primitives::icon;

use super::rail_tooltip::{HoverPlace, RowKey, Target};
use super::{DismissTip, Entry, Sidebar, SidebarEvent};
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
    key: RowKey,
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
                key: RowKey::NewCompare,
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
                key: RowKey::Tab(tab.index),
            },
            m,
            cx,
        )
        .aria_selected(selected)
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
            key,
        } = control;
        let (ring, hover) = (m.accent, m.accent_tint);
        let ring_w = px(SidewiseTheme::global(cx).spec.ring_px);
        let tip = Target::new(key, entry.label.clone());
        let row = self
            .row_view(entry, m, cx)
            .id(id)
            .role(role)
            .aria_label(entry.label.clone())
            .test_support()
            .track_focus(&focus)
            .rounded(DIMS.btn_r)
            .border(ring_w)
            .relative()
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
            });
        self.attach_rail_tip(row, tip, cx)
    }

    /// In the rail, adds row hover to the tooltip state and a bounds probe while the row is active.
    fn attach_rail_tip(
        &self,
        row: ObservedElement<Stateful<Div>>,
        tip: Target,
        cx: &mut Context<Self>,
    ) -> ObservedElement<Stateful<Div>> {
        if !self.rail {
            return row;
        }
        let key = tip.key;
        let row = row
            .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.set_tip_hover((HoverPlace::Row, tip.clone()), *hovered, cx);
            }));
        if !self.tips.wants_bounds(key) {
            return row;
        }
        let (this, ring_w) = (cx.entity(), px(SidewiseTheme::global(cx).spec.ring_px));
        let probe = canvas(
            move |bounds, _, cx| {
                // The padding box inside the border; dilate restores the row's outer bounds.
                let outer = bounds.dilate(ring_w);
                this.update(cx, |sb, cx| sb.set_tip_bounds(key, outer, cx));
            },
            |_, _, _, _| {},
        );
        row.child(probe.absolute().inset_0())
    }

    /// Records one hover change in the tooltip state and repaints when the shown tip changed.
    fn set_tip_hover(
        &mut self,
        (place, tip): (HoverPlace, Target),
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        if self.tips.set_hovered(place, tip, hovered) {
            cx.notify();
        }
    }

    /// Records a row's measured outer bounds and repaints when they changed.
    fn set_tip_bounds(&mut self, key: RowKey, outer: Bounds<Pixels>, cx: &mut Context<Self>) {
        if self.tips.set_bounds(key, outer) {
            cx.notify();
        }
    }

    /// The row target holding keyboard focus in the rail, or nothing outside it.
    fn focused_row_target(&self, window: &Window) -> Option<Target> {
        if !self.rail {
            return None;
        }
        if self.new_compare_focus.is_focused(window) {
            return Some(Target::new(RowKey::NewCompare, "New comparison"));
        }
        let ix = self.tab_focuses.iter().position(|h| h.is_focused(window))?;
        Some(Target::new(RowKey::Tab(ix), self.tabs.get(ix)?.clone()))
    }

    /// Mirrors the focused row into the tooltip state before the frame is built.
    pub(super) fn sync_rail_focus(&mut self, window: &Window) {
        let focused = self.focused_row_target(window);
        self.tips.sync_focus(focused);
    }

    /// The deferred tooltip overlay; pointer movement over it feeds overlay hover.
    pub(super) fn rail_tip_overlay(
        &self,
        m: &ModeTokens,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let this = cx.entity();
        self.tips.overlay(
            move |tip, hovered, _, cx| {
                this.update(cx, |sb, cx| {
                    sb.set_tip_hover((HoverPlace::Overlay, tip), hovered, cx)
                });
            },
            m,
        )
    }

    /// `Escape` inside the sidebar: consumed when a tooltip was dismissed, else passed on.
    pub(super) fn on_dismiss_tip(
        &mut self,
        _: &DismissTip,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.dismiss_tip(cx) {
            cx.propagate();
        }
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

impl Sidebar {
    /// Replaces the Recent and Named rows and redraws.
    pub fn set_rows(&mut self, rows: StartRows, cx: &mut Context<Self>) {
        self.rows = rows;
        cx.notify();
    }

    /// The rows shown (lets the shell tests read them).
    #[cfg(test)]
    pub(crate) fn rows(&self) -> &StartRows {
        &self.rows
    }
}
