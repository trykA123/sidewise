//! Check of the full-window file diff's context menu: opening it must put its rows on screen.

use std::fs;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, TestAppContext};

use crate::compare_tab_script::scripted_shell;
use crate::compare_tab_tests::{TempDir, submit, wait_for_row};

/// The ShowMenu action opens the diff menu state (asserted first), but nothing draws it.
#[gpui_kit::test]
fn the_file_diff_menu_action_draws_the_menu(cx: &mut TestAppContext) {
    let left = TempDir::new("dm-l");
    let right = TempDir::new("dm-r");
    fs::write(left.path().join("a.txt"), "one").expect("write left");
    fs::write(right.path().join("a.txt"), "two").expect("write right");
    let opened = scripted_shell(cx);
    submit(cx, &opened, [left.path(), right.path()]);
    assert!(wait_for_row(cx, &opened, "a.txt"), "rows reach the view");
    cx.simulate_keystrokes(opened.0, "enter");
    cx.run_until_parked();
    cx.update_window(opened.0, |_, window, cx| {
        window.dispatch_action(Box::new(sidewise_ui::commands::compare::ShowMenu), cx);
    })
    .expect("window");
    cx.run_until_parked();
    let tab = opened
        .1
        .read_with(cx, |shell, _| shell.active_compare().cloned());
    let screen = tab
        .expect("compare tab")
        .read_with(cx, |tab, _| tab.screen().clone());
    let state = screen.read_with(cx, |screen, _| screen.compare_menu_view().is_some());
    assert!(state, "the action opened the menu state");
    let drawn = cx
        .update_window(opened.0, |_, window, _| {
            window.try_find(("menu-row", 0usize)).is_some()
        })
        .expect("window");
    assert!(drawn, "the diff menu has a first row on screen");
}
