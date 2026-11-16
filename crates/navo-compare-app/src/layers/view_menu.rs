//! The View menu popover (U16b): the Simple/Rich/Custom presets, layout, six toggles and the
//! kind of view. It edits a copy of `UiPrefs`; the host saves it on `Changed` and closes on `Dismissed`.

use gpui_kit::{EventEmitter, FocusHandle, KeyDownEvent};
use navo_compare_profile::appearance::{UiPrefs, ViewModePref, ViewPresetPref};
use navo_ui::prelude::*;

/// The persistent note under the kind of view.
pub const KIND_NOTE: &str =
    "Every pair opens. A kind that fails or passes its cap steps down to Bytes with a notice.";
/// The notice shown when a pair has no text view and the kind steps down to Bytes.
pub const STEP_DOWN_NOTICE: &str = "No text view for this pair. Alt+V opens it as Bytes.";

/// How a text diff is laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Split,
    Unified,
}

/// The six individual View toggles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Words,
    Syntax,
    LineNo,
    Fold,
    Wrap,
    IgnoreWhitespace,
}

/// What the host should do after a menu change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMenuEvent {
    /// A preference changed; save and apply it.
    Changed,
    /// Close the menu and give focus back (Escape, or a kind was picked).
    Dismissed,
}

/// One menu entry's meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    Preset(ViewPresetPref),
    Layout(Layout),
    Toggle(Toggle),
    Kind(ViewModePref),
}

/// The entries in keyboard order: presets, layouts, toggles, kinds.
#[rustfmt::skip]
const ITEMS: [(Act, &str, &str); 14] = [
    (Act::Preset(ViewPresetPref::Simple), "Simple", ""), (Act::Preset(ViewPresetPref::Rich), "Rich", ""),
    (Act::Preset(ViewPresetPref::Custom), "Custom", ""),
    (Act::Layout(Layout::Split), "Side by side", "l"), (Act::Layout(Layout::Unified), "Single file · unified", "l"),
    (Act::Toggle(Toggle::Words), "Changed words inside a line", ""), (Act::Toggle(Toggle::Syntax), "Syntax colours", ""),
    (Act::Toggle(Toggle::LineNo), "Line numbers", ""), (Act::Toggle(Toggle::Fold), "Fold unchanged lines", ""),
    (Act::Toggle(Toggle::Wrap), "Wrap long lines", "alt-z"), (Act::Toggle(Toggle::IgnoreWhitespace), "Ignore whitespace", ""),
    (Act::Kind(ViewModePref::Best), "Best view for this pair", ""), (Act::Kind(ViewModePref::Text), "Text", ""),
    (Act::Kind(ViewModePref::Bytes), "Bytes", ""),
];

/// The View menu state: the edited preferences, the layout and the keyboard-active entry.
pub struct ViewMenu {
    prefs: UiPrefs,
    layout: Layout,
    active: usize,
    handle: FocusHandle,
    /// Whether the last frame ran the opacity entrance; false under reduced motion.
    animated: std::cell::Cell<bool>,
}

impl EventEmitter<ViewMenuEvent> for ViewMenu {}

impl ViewMenu {
    /// A menu over `prefs`; the layout follows the preset (Simple is unified, the rest split).
    pub fn new(prefs: UiPrefs, cx: &mut Context<Self>) -> Self {
        let layout = layout_of(prefs.preset != ViewPresetPref::Simple);
        let active = ITEMS
            .iter()
            .position(|item| item.0 == Act::Preset(prefs.preset))
            .unwrap_or(0);
        Self {
            prefs,
            layout,
            active,
            handle: cx.focus_handle(),
            animated: std::cell::Cell::new(false),
        }
    }

    /// The preferences as edited so far.
    pub fn prefs(&self) -> &UiPrefs {
        &self.prefs
    }

    /// The current layout.
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Moves focus into the menu, so arrows, Enter and Escape reach it.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.handle.focus(window, cx);
    }

    /// The notice for a kind that cannot show: `Best` and `Text` step down to Bytes when the pair has no text view.
    pub fn step_down(&self, text_ok: bool) -> Option<&'static str> {
        (self.prefs.view_mode != ViewModePref::Bytes && !text_ok).then_some(STEP_DOWN_NOTICE)
    }

    /// Applies one entry: a preset sets its toggles, anything else by hand makes the preset Custom.
    fn apply(&mut self, act: Act, cx: &mut Context<Self>) {
        let p = &mut self.prefs;
        match act {
            Act::Preset(preset) => {
                p.preset = preset;
                if preset != ViewPresetPref::Custom {
                    let rich = preset == ViewPresetPref::Rich;
                    (p.words, p.syntax, p.fold_unchanged) = (rich, rich, rich);
                    self.layout = layout_of(rich);
                }
            }
            Act::Layout(layout) => (self.layout, p.preset) = (layout, ViewPresetPref::Custom),
            Act::Toggle(toggle) => {
                *flag_mut(p, toggle) ^= true;
                p.preset = ViewPresetPref::Custom;
            }
            Act::Kind(kind) => p.view_mode = kind,
        }
        cx.emit(ViewMenuEvent::Changed);
        if matches!(act, Act::Kind(_)) {
            cx.emit(ViewMenuEvent::Dismissed);
        }
        cx.notify();
    }

    /// Whether an entry is the chosen preset, layout or kind, or an enabled toggle.
    fn is_on(&self, act: Act) -> bool {
        match act {
            Act::Preset(p) => self.prefs.preset == p,
            Act::Layout(l) => self.layout == l,
            Act::Kind(k) => self.prefs.view_mode == k,
            Act::Toggle(t) => match t {
                Toggle::Words => self.prefs.words,
                Toggle::Syntax => self.prefs.syntax,
                Toggle::LineNo => self.prefs.line_no,
                Toggle::Fold => self.prefs.fold_unchanged,
                Toggle::Wrap => self.prefs.wrap_lines,
                Toggle::IgnoreWhitespace => self.prefs.ignore_whitespace,
            },
        }
    }

    /// Arrow keys move the active entry (wrapping), Enter or Space applies it, Escape dismisses.
    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        let n = ITEMS.len();
        match (stroke.modifiers.modified(), stroke.key.as_str()) {
            (false, "down") => self.active = (self.active + 1) % n,
            (false, "up") => self.active = (self.active + n - 1) % n,
            (false, "home") => self.active = 0,
            (false, "end") => self.active = n - 1,
            (false, "enter" | "space") => return self.apply(ITEMS[self.active].0, cx),
            (false, "escape") => cx.emit(ViewMenuEvent::Dismissed),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }
}

/// The preference field behind one toggle.
fn flag_mut(prefs: &mut UiPrefs, toggle: Toggle) -> &mut bool {
    match toggle {
        Toggle::Words => &mut prefs.words,
        Toggle::Syntax => &mut prefs.syntax,
        Toggle::LineNo => &mut prefs.line_no,
        Toggle::Fold => &mut prefs.fold_unchanged,
        Toggle::Wrap => &mut prefs.wrap_lines,
        Toggle::IgnoreWhitespace => &mut prefs.ignore_whitespace,
    }
}

/// Rich shows side by side; Simple shows one unified file.
fn layout_of(rich: bool) -> Layout {
    if rich { Layout::Split } else { Layout::Unified }
}

#[path = "view_menu_view.rs"]
mod view;

#[cfg(test)]
#[path = "view_menu_tests.rs"]
mod tests;
