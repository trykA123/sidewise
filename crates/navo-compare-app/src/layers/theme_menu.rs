//! The Theme picker's state (NC150, NC151): five palettes and three modes in one list. The host
//! saves the choice on `Changed` and closes the picker on `Dismissed`.

use gpui_kit::{EventEmitter, FocusHandle};
use navo_compare_profile::appearance::{AppearanceMode, PaletteId};
use sidewise_kit::prelude::*;

/// One picker row's meaning: a palette or a mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Palette(PaletteId),
    Mode(AppearanceMode),
}

/// What the host should do after a picker change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMenuEvent {
    /// The palette or mode changed; save and apply it.
    Changed,
    /// Close the picker and give focus back (Escape).
    #[allow(dead_code, reason = "emitted by the key handler in TH5")]
    Dismissed,
}

/// Keyboard order: the five palettes in `PaletteId` order, then the three modes.
#[rustfmt::skip]
pub(crate) const ITEMS: [(Pick, &str); 8] = [
    (Pick::Palette(PaletteId::Violet), "Violet"), (Pick::Palette(PaletteId::Graphite), "Graphite"),
    (Pick::Palette(PaletteId::Ocean), "Ocean"), (Pick::Palette(PaletteId::Ember), "Ember"),
    (Pick::Palette(PaletteId::Contrast), "High contrast"),
    (Pick::Mode(AppearanceMode::System), "System"), (Pick::Mode(AppearanceMode::Light), "Light"),
    (Pick::Mode(AppearanceMode::Dark), "Dark"),
];

/// The picker state: the chosen palette and mode, and the keyboard-active row.
pub struct ThemeMenu {
    palette: PaletteId,
    mode: AppearanceMode,
    active: usize,
    #[allow(dead_code, reason = "read by `focus` and the view in TH5")]
    handle: FocusHandle,
}

impl EventEmitter<ThemeMenuEvent> for ThemeMenu {}

impl ThemeMenu {
    /// A picker over the current `palette` and `mode`; the active row is the palette's row.
    pub fn new(palette: PaletteId, mode: AppearanceMode, cx: &mut Context<Self>) -> Self {
        let active = ITEMS
            .iter()
            .position(|item| item.0 == Pick::Palette(palette))
            .unwrap_or(0);
        Self {
            palette,
            mode,
            active,
            handle: cx.focus_handle(),
        }
    }

    /// The palette and mode as chosen so far.
    pub fn choice(&self) -> (PaletteId, AppearanceMode) {
        (self.palette, self.mode)
    }

    #[allow(dead_code, reason = "called by the shell host in TH6")]
    /// Moves focus into the picker, so arrows, Enter and Escape reach it.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.handle.focus(window, cx);
    }

    /// Whether `pick` is the current palette or mode.
    fn is_on(&self, pick: Pick) -> bool {
        match pick {
            Pick::Palette(p) => p == self.palette,
            Pick::Mode(m) => m == self.mode,
        }
    }

    /// Sets the pick; emits `Changed` and notifies only when the value changes.
    fn apply(&mut self, pick: Pick, cx: &mut Context<Self>) {
        if self.is_on(pick) {
            return;
        }
        match pick {
            Pick::Palette(p) => self.palette = p,
            Pick::Mode(m) => self.mode = m,
        }
        cx.emit(ThemeMenuEvent::Changed);
        cx.notify();
    }
}

#[cfg(test)]
#[path = "theme_menu_tests.rs"]
mod tests;
