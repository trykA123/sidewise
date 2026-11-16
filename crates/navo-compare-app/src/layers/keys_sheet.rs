//! The Keys sheet (`?` or F1): a modal list of every shortcut with a live filter.
//! The shell routes its actions through `wire`.

use gpui_kit::component::input::InputEvent;
use gpui_kit::{FocusHandle, KeyBinding, ReadGlobal, Subscription};
use navo_ui::commands::{COMPARE_KEYMAP, Keymap, shown};
use navo_ui::prelude::*;
use navo_ui::primitives::{Kbd, TextField, TextFieldState, text_field_state};

use crate::theme::SidewiseTheme;
use crate::theme_data::palettes::ModeTokens;

gpui_kit::actions!(layers, [ToggleKeys, CloseKeys]);

/// Key context of the open sheet; Escape is bound inside it.
const SHEET: &str = "KeysSheet";
/// Sidewise's own keys, shown first; the Compare keys come from `COMPARE_KEYMAP`.
const WINDOW_KEYS: [(&str, &[&str]); 5] = [
    ("New tab", &["ctrl-t"]),
    ("Close tab", &["ctrl-w"]),
    ("Next tab", &["ctrl-tab"]),
    ("Previous tab", &["ctrl-shift-tab"]),
    ("Show keyboard shortcuts", &["?", "f1"]),
];

/// One line of the sheet: a command's label and its keys in gpui form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyRow {
    pub label: SharedString,
    pub keys: Vec<String>,
}

/// A titled run of rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyGroup {
    pub title: &'static str,
    pub rows: Vec<KeyRow>,
}

/// Binds `?` and F1 outside text fields, and Escape inside the open sheet.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("?", ToggleKeys, Some("Shell && !Input")),
        KeyBinding::new("f1", ToggleKeys, Some("Shell")),
        KeyBinding::new("escape", CloseKeys, Some(SHEET)),
    ]);
}

/// Hooks the open and close actions onto `root`, so the shell opens the sheet.
pub fn wire(root: Div, keys: &Entity<KeysSheet>) -> Div {
    let sheet = keys.clone();
    root.on_action(move |_: &ToggleKeys, window, cx| {
        sheet.update(cx, |sheet, cx| sheet.toggle(window, cx));
    })
}

/// The group title of a keymap context.
fn title_of(context: &str) -> &'static str {
    match context {
        "CompareView" => "File diff",
        "CompareStream" => "Stream",
        "CompareOutline" => "Outline",
        _ => "Compare",
    }
}

/// Sidewise's keys, then the Compare keys grouped by context.
pub fn key_groups() -> Vec<KeyGroup> {
    let row = |label: &str, keys: &[&str]| KeyRow {
        label: label.to_owned().into(),
        keys: keys.iter().map(|key| (*key).to_owned()).collect(),
    };
    let window = WINDOW_KEYS.map(|(label, keys)| row(label, keys));
    let mut groups = vec![KeyGroup {
        title: "Window",
        rows: window.to_vec(),
    }];
    let Ok(keymap) = Keymap::parse(COMPARE_KEYMAP) else {
        return groups;
    };
    for bind in keymap.binds() {
        let title = title_of(&bind.context);
        let at = groups.iter().position(|group| group.title == title);
        let at = at.unwrap_or_else(|| {
            groups.push(KeyGroup {
                title,
                rows: Vec::new(),
            });
            groups.len() - 1
        });
        let keys: Vec<&str> = bind.keys.iter().map(String::as_str).collect();
        groups[at].rows.push(row(bind.spec.label, &keys));
    }
    groups
}

/// The rows whose label or shown keys contain `query` (case-insensitive); empty groups drop out.
pub fn filter_groups(groups: &[KeyGroup], query: &str) -> Vec<KeyGroup> {
    let query = query.trim().to_lowercase();
    let hit = |row: &KeyRow| {
        row.label.to_lowercase().contains(&query)
            || row
                .keys
                .iter()
                .any(|key| shown(key).is_some_and(|text| text.to_lowercase().contains(&query)))
    };
    groups
        .iter()
        .filter_map(|group| {
            let rows: Vec<KeyRow> = group.rows.iter().filter(|row| hit(row)).cloned().collect();
            (!rows.is_empty()).then_some(KeyGroup {
                title: group.title,
                rows,
            })
        })
        .collect()
}

/// The active mode's tokens; the drawer host reads them too.
pub(super) fn tokens_of(cx: &App) -> (SidewiseTheme, ModeTokens) {
    let side = *SidewiseTheme::global(cx);
    let tones = if side.mode.is_dark() {
        side.spec.dark
    } else {
        side.spec.light
    };
    (side, tones)
}

/// The Keys sheet: a centred modal over the scrim with a live shortcut filter.
pub struct KeysSheet {
    open: bool,
    groups: Vec<KeyGroup>,
    query: Entity<TextFieldState>,
    /// Where focus was before the sheet opened; Escape gives it back.
    restore: Option<FocusHandle>,
    _typing: Subscription,
}

impl KeysSheet {
    /// A closed sheet over `key_groups()`.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = text_field_state("Filter shortcuts", window, cx);
        let typing = cx.subscribe(&query, |_, _, _: &InputEvent, cx| cx.notify());
        Self {
            open: false,
            groups: key_groups(),
            query,
            restore: None,
            _typing: typing,
        }
    }

    /// Whether the sheet is showing.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens a closed sheet (remembering focus, focusing the filter) or closes an open one.
    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close(window, cx);
            return;
        }
        self.restore = window.focused(cx);
        self.open = true;
        self.query.update(cx, |field, cx| field.focus(window, cx));
        cx.notify();
    }

    /// Closes the sheet and returns focus to where it was.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        if let Some(handle) = self.restore.take() {
            window.focus(&handle, cx);
        }
        cx.notify();
    }

    /// The scrolling list of groups that match the filter.
    fn body(&self, m: &ModeTokens, cx: &App) -> impl IntoElement {
        let groups = filter_groups(&self.groups, &self.query.read(cx).value());
        let group_view = |(g, group): (usize, KeyGroup)| {
            let rows = group.rows.into_iter().enumerate().map(move |(r, row)| {
                let id = |k: usize| SharedString::from(format!("key-{g}-{r}-{k}"));
                let kbds: Vec<_> = (row.keys.iter().enumerate())
                    .filter_map(|(k, key)| Kbd::new(id(k), key))
                    .collect();
                (div().flex().justify_between().gap_3().py_1())
                    .child(row.label)
                    .child(div().flex().gap_1().children(kbds))
            });
            let title = div().text_color(m.muted).child(group.title);
            div().pb_3().child(title).children(rows)
        };
        let items = groups.into_iter().enumerate().map(group_view);
        div()
            .id("keys-body")
            .flex_1()
            .overflow_y_scroll()
            .children(items)
    }
}

impl Render for KeysSheet {
    /// Draws nothing while closed; open, the scrim with the 22 px card (NC68: solid, no blur).
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let (side, m) = tokens_of(cx);
        let card = div()
            .id("keys-card")
            .key_context(SHEET)
            .on_action(cx.listener(|this, _: &CloseKeys, window, cx| this.close(window, cx)))
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .flex()
            .flex_col()
            .gap_3()
            .w(px(520.))
            .max_h(px(560.))
            .p_4()
            .rounded(side.dims.sheet_r)
            .border_1()
            .border_color(m.sep_strong)
            .bg(m.card)
            .text_color(m.text)
            .shadow_lg()
            .child("Keyboard shortcuts")
            .child(TextField::new(
                "keys-filter",
                &self.query,
                "Filter shortcuts",
            ))
            .child(self.body(&m, cx));
        (div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center())
        .bg(m.scrim)
        .child(card)
        .into_any_element()
    }
}

#[cfg(test)]
#[path = "keys_sheet_tests.rs"]
mod tests;
