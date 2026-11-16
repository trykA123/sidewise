//! Target selection for collapsed-rail tooltips across row hover, overlay hover, and focus.
//! Pure selection state gated under `#[cfg(test)]` until wired into `Sidebar`.

use gpui_kit::SharedString;

/// Stable identity of an interactive sidebar row eligible for a rail tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RowKey {
    NewCompare,
    Tab(usize),
}

/// Which interactive surface reported pointer hover for a rail tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HoverPlace {
    Row,
    Overlay,
}

/// Owned row identity and full tooltip text carried across hover and focus updates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub(super) key: RowKey,
    pub(super) label: SharedString,
}

impl Target {
    /// Creates an owned tooltip target from `key` and its display `label`.
    pub(super) fn new(key: RowKey, label: impl Into<SharedString>) -> Self {
        Self {
            key,
            label: label.into(),
        }
    }
}

/// Tracks active hover and keyboard focus sources for collapsed-rail tooltips.
#[derive(Default)]
pub(super) struct RailTips {
    row_hover: Option<Target>,
    overlay_hover: Option<Target>,
    focused: Option<Target>,
}

impl RailTips {
    /// Creates empty tooltip selection state with no hovered or focused target.
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Clears all hovered and focused targets (used when expanding the rail).
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Updates row or overlay hover and returns `true` when `active_target` changes.
    pub(super) fn set_hovered(&mut self, place: HoverPlace, target: Target, hovered: bool) -> bool {
        let before = self.active_target().cloned();
        if hovered
            && place == HoverPlace::Row
            && self
                .overlay_hover
                .as_ref()
                .is_some_and(|cur| cur.key != target.key)
        {
            self.overlay_hover = None;
        }
        let slot = match place {
            HoverPlace::Row => &mut self.row_hover,
            HoverPlace::Overlay => &mut self.overlay_hover,
        };
        if hovered {
            *slot = Some(target);
        } else if slot.as_ref().is_some_and(|cur| cur.key == target.key) {
            *slot = None;
        }
        self.active_target() != before.as_ref()
    }

    /// Replaces the currently keyboard-focused row target.
    pub(super) fn sync_focus(&mut self, focused: Option<Target>) {
        self.focused = focused;
    }

    /// Active target selected by precedence (`row_hover > overlay_hover > focused`).
    pub(super) fn active_target(&self) -> Option<&Target> {
        self.row_hover
            .as_ref()
            .or(self.overlay_hover.as_ref())
            .or(self.focused.as_ref())
    }
}

#[cfg(test)]
#[path = "rail_tooltip_tests.rs"]
mod tests;
