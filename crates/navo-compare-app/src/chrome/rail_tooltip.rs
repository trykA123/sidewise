//! State for collapsed-rail tooltips: hover priority, Esc suppression, anchor bounds, and pruning.
//! Pure state gated under `#[cfg(test)]` until overlay rendering (A2) and `Sidebar` wiring (B).

use gpui_kit::{Bounds, Pixels, SharedString, px};

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

/// Tracks active hover, keyboard focus, measured bounds, and `Escape` suppression for tooltips.
#[derive(Default)]
pub(super) struct RailTips {
    row_hover: Option<Target>,
    overlay_hover: Option<Target>,
    focused: Option<Target>,
    suppressed: Option<RowKey>,
    anchor: Option<(RowKey, Bounds<Pixels>)>,
}

impl RailTips {
    /// Creates empty tooltip state with no hovered, focused, suppressed, or anchored target.
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Clears all hovered, focused, suppressed, and anchored state (used when expanding the rail).
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Updates row or overlay hover and returns `true` when the displayed target changes.
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
        self.normalize();
        self.active_target() != before.as_ref()
    }

    /// Replaces the currently keyboard-focused row target and normalizes suppression/anchor state.
    pub(super) fn sync_focus(&mut self, focused: Option<Target>) {
        self.focused = focused;
        self.normalize();
    }

    /// Clears overlay/anchor, prunes removed or relabeled tabs, and normalizes suppression.
    pub(super) fn sync_tabs(&mut self, tabs: &[SharedString]) {
        self.overlay_hover = None;
        self.anchor = None;
        let matches = |t: &Target| match t.key {
            RowKey::NewCompare => true,
            RowKey::Tab(ix) => tabs.get(ix) == Some(&t.label),
        };
        if self.row_hover.as_ref().is_some_and(|t| !matches(t)) {
            self.row_hover = None;
        }
        if self.focused.as_ref().is_some_and(|t| !matches(t)) {
            self.focused = None;
        }
        self.normalize();
    }

    /// Suppresses the active target, drops stale overlay hover/anchor, and reports dismissal.
    pub(super) fn dismiss(&mut self) -> bool {
        let Some(key) = self.active_target().map(|t| t.key) else {
            return false;
        };
        self.suppressed = Some(key);
        self.overlay_hover = None;
        self.anchor = None;
        self.normalize();
        true
    }

    /// Returns whether `key` is the active non-suppressed target needing bounds capture.
    pub(super) fn wants_bounds(&self, key: RowKey) -> bool {
        self.active_target().is_some_and(|t| t.key == key)
    }

    /// Records positive bounds for the active non-suppressed target; returns `true` on change.
    pub(super) fn set_bounds(&mut self, key: RowKey, bounds: Bounds<Pixels>) -> bool {
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) || !self.wants_bounds(key) {
            return false;
        }
        let next = (key, bounds);
        if self.anchor == Some(next) {
            return false;
        }
        self.anchor = Some(next);
        true
    }

    /// Active target with matching positive anchor bounds for A2 overlay rendering.
    fn active_anchor(&self) -> Option<(&Target, Bounds<Pixels>)> {
        let target = self.active_target()?;
        let (key, bounds) = self.anchor?;
        (key == target.key).then_some((target, bounds))
    }

    /// Active visible target (`row_hover > overlay_hover > focused`), hidden while suppressed.
    pub(super) fn active_target(&self) -> Option<&Target> {
        self.row_hover
            .as_ref()
            .or(self.overlay_hover.as_ref())
            .or(self.focused.as_ref())
            .filter(|t| self.suppressed != Some(t.key))
    }

    /// Releases exited suppression and drops `anchor` when no longer matching the active key.
    fn normalize(&mut self) {
        if let Some(sup) = self.suppressed {
            let live = [&self.row_hover, &self.overlay_hover, &self.focused]
                .into_iter()
                .flatten()
                .any(|t| t.key == sup);
            if !live {
                self.suppressed = None;
            }
        }
        if self.active_target().map(|t| t.key) != self.anchor.map(|(k, _)| k) {
            self.anchor = None;
        }
    }
}

#[cfg(test)]
#[path = "rail_tooltip_tests.rs"]
mod tests;
