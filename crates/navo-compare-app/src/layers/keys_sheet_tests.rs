//! Tests of the Keys sheet: key groups, the live filter, the keys that open it and focus
//! restore on Escape.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, FocusHandle, TestAppContext};
use navo_compare_profile::appearance::PaletteId;
use navo_ui::tokens::Mode;

use super::*;
use crate::test_window::open;
use crate::theme::apply_sidewise;

/// A stand-in shell: the `Shell` context over a focusable body, and the sheet.
struct Root {
    keys: Entity<KeysSheet>,
    body: FocusHandle,
}

impl Render for Root {
    /// Wires the actions the way the shell will and mounts the sheet beside the body.
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let body = div().id("body").track_focus(&self.body).flex_1();
        let root = div().key_context("Shell").size_full().flex().child(body);
        wire(root, &self.keys).child(self.keys.clone())
    }
}

/// A window holding `Root` with the body focused and the keys bound.
fn root(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Root>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        apply_sidewise(PaletteId::Violet, Mode::Light, cx);
        bind_keys(cx);
    });
    let (window, root) = open(cx, |window, cx| {
        let root = Root {
            keys: cx.new(|cx| KeysSheet::new(window, cx)),
            body: cx.focus_handle(),
        };
        window.focus(&root.body, cx);
        root
    });
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .expect("window");
    (window, root)
}

/// The sheet of a root.
fn sheet(cx: &mut TestAppContext, root: &Entity<Root>) -> Entity<KeysSheet> {
    root.read_with(cx, |root, _| root.keys.clone())
}

/// The labels of the sheet's rows that match `query`, in order.
fn labels(query: &str) -> Vec<SharedString> {
    let groups = filter_groups(&key_groups(), query);
    groups
        .iter()
        .flat_map(|group| group.rows.iter().map(|row| row.label.clone()))
        .collect()
}

/// The groups hold the Sidewise keys and the Compare keys, and none of the dropped ones.
#[gpui_kit::test]
fn the_groups_come_from_the_compare_keymap_and_sidewise(_cx: &mut TestAppContext) {
    let groups = key_groups();
    let titles: Vec<_> = groups.iter().map(|group| group.title).collect();
    assert_eq!(titles[0], "Window", "Sidewise keys come first");
    assert!(
        titles.contains(&"File diff") && titles.contains(&"Stream"),
        "{titles:?}"
    );
    let keys: Vec<&str> = groups
        .iter()
        .flat_map(|group| {
            group
                .rows
                .iter()
                .flat_map(|row| row.keys.iter().map(String::as_str))
        })
        .collect();
    for want in ["?", "f1", "ctrl-t", "ctrl-b", "ctrl-i", "alt-down"] {
        assert!(keys.contains(&want), "missing {want}");
    }
    for dropped in ["t", "shift-t", "6"] {
        assert!(!keys.contains(&dropped), "{dropped} must not be listed");
    }
}

/// The filter matches labels and shown keys, case-insensitively, and drops empty groups.
#[gpui_kit::test]
fn the_filter_matches_labels_and_keys(_cx: &mut TestAppContext) {
    assert_eq!(labels("NEW TAB"), ["New tab"], "label, any case");
    assert!(
        labels("ctrl+b").iter().any(|label| label.contains("hange")),
        "shown key"
    );
    assert!(labels("no such command").is_empty());
    let groups = filter_groups(&key_groups(), "close tab");
    assert_eq!(groups.len(), 1, "groups without a match are dropped");
}

/// `?` and F1 open the sheet and Escape closes it and gives focus back to the body.
#[gpui_kit::test]
fn keys_open_the_sheet_and_escape_restores_focus(cx: &mut TestAppContext) {
    let (window, root_view) = root(cx);
    let keys = sheet(cx, &root_view);
    for key in ["?", "f1"] {
        cx.simulate_keystrokes(window, key);
        assert!(
            keys.read_with(cx, |sheet, _| sheet.is_open()),
            "{key} opens the sheet"
        );
        cx.simulate_keystrokes(window, "escape");
        assert!(
            !keys.read_with(cx, |sheet, _| sheet.is_open()),
            "escape closes after {key}"
        );
        let back = cx
            .update_window(window, |_, window, cx| {
                root_view.read(cx).body.is_focused(window)
            })
            .expect("window");
        assert!(back, "focus returns to the body after {key}");
    }
}

/// Typing in the filter field is text: a `?` does not close the sheet and lands in the field.
#[gpui_kit::test]
fn a_typed_question_mark_stays_text_in_the_filter(cx: &mut TestAppContext) {
    let (window, root_view) = root(cx);
    let keys = sheet(cx, &root_view);
    cx.simulate_keystrokes(window, "f1");
    cx.simulate_input(window, "?");
    assert!(
        keys.read_with(cx, |sheet, _| sheet.is_open()),
        "a typed ? stays text"
    );
    let typed = keys.read_with(cx, |sheet, cx| sheet.query.read(cx).value().to_string());
    assert_eq!(typed, "?", "the field holds the typed text");
}

/// The sheet draws open, with its scrim, card, filter and rows.
#[gpui_kit::test]
fn the_open_sheet_draws(cx: &mut TestAppContext) {
    let (window, root_view) = root(cx);
    cx.simulate_keystrokes(window, "f1");
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .expect("window");
    assert!(sheet(cx, &root_view).read_with(cx, |sheet, _| sheet.is_open()));
}
