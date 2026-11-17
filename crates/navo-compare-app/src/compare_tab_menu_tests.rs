//! Checks of the compare screen's context menu: opening it on the file diff or the Stream puts
//! its rows on screen, Shift+F10 opens it, and Esc closes it with the focus back on the diff.

use std::fs;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use sidewise_ui::commands::compare::ShowMenu;
use sidewise_ui::window_view::CompareScreen;

use crate::compare_tab_script::scripted_shell;
use crate::compare_tab_tests::{TempDir, submit, wait_for_row};
use crate::screens::compare_tab::CompareTab;
use crate::shell::Shell;

type Opened = (AnyWindowHandle, Entity<Shell>);

/// Opens a shell on two folders with a differing `a.txt` and waits for its row.
fn walked(cx: &mut TestAppContext, tag: &str) -> (Opened, [TempDir; 2]) {
    let left = TempDir::new(&format!("{tag}-l"));
    let right = TempDir::new(&format!("{tag}-r"));
    fs::write(left.path().join("a.txt"), "one").expect("write left");
    fs::write(right.path().join("a.txt"), "two").expect("write right");
    let opened = scripted_shell(cx);
    submit(cx, &opened, [left.path(), right.path()]);
    assert!(wait_for_row(cx, &opened, "a.txt"), "rows reach the view");
    (opened, [left, right])
}

/// The active compare tab of the shell.
fn tab(cx: &mut TestAppContext, opened: &Opened) -> Entity<CompareTab> {
    (opened.1)
        .read_with(cx, |shell, _| shell.active_compare().cloned())
        .expect("compare tab")
}

/// The compare screen of the active tab.
fn screen(cx: &mut TestAppContext, opened: &Opened) -> Entity<CompareScreen> {
    tab(cx, opened).read_with(cx, |tab, _| tab.screen().clone())
}

/// Dispatches the menu action on the focused element and lets the menu open.
fn show_menu(cx: &mut TestAppContext, opened: &Opened) {
    cx.update_window(opened.0, |_, window, cx| {
        window.dispatch_action(Box::new(ShowMenu), cx);
    })
    .expect("window");
    cx.run_until_parked();
}

/// Whether the menu state is open on the screen.
fn menu_open(cx: &mut TestAppContext, opened: &Opened) -> bool {
    screen(cx, opened).read_with(cx, |screen, _| screen.compare_menu_view().is_some())
}

/// Whether the first menu row is drawn in a fresh frame.
fn menu_drawn(cx: &mut TestAppContext, opened: &Opened) -> bool {
    cx.update_window(opened.0, |_, window, cx| {
        window.render_frame(cx);
        window.try_find(("menu-row", 0usize)).is_some()
    })
    .expect("window")
}

/// The ShowMenu action on the file diff opens the menu and draws its rows.
#[gpui_kit::test]
fn the_file_diff_menu_action_draws_the_menu(cx: &mut TestAppContext) {
    let (opened, _dirs) = walked(cx, "dm");
    cx.simulate_keystrokes(opened.0, "enter");
    cx.run_until_parked();
    show_menu(cx, &opened);
    assert!(menu_open(cx, &opened), "the action opened the menu state");
    assert!(
        menu_drawn(cx, &opened),
        "the diff menu has a first row on screen"
    );
}

/// The ShowMenu action on the Stream opens the menu and draws its rows.
#[gpui_kit::test]
fn the_stream_menu_action_draws_the_menu(cx: &mut TestAppContext) {
    use crate::screens::folders_view::FoldersEvent;
    let (opened, _dirs) = walked(cx, "sm");
    let tab = tab(cx, &opened);
    let id = tab
        .read_with(cx, |tab, _| tab.row_id("a.txt"))
        .expect("row");
    let folders = tab.read_with(cx, |tab, _| tab.folders().clone());
    cx.update_window(opened.0, |_, _, cx| {
        folders.update(cx, |_, cx| cx.emit(FoldersEvent::OpenStream(id)));
    })
    .expect("window");
    cx.run_until_parked();
    show_menu(cx, &opened);
    assert!(menu_open(cx, &opened), "the action opened the Stream menu");
    assert!(
        menu_drawn(cx, &opened),
        "the Stream menu has a first row on screen"
    );
}

/// With the app's compare keys bound, Shift+F10 and the Menu key open the diff menu; Esc
/// closes it and gives the focus back to the file diff.
#[gpui_kit::test]
fn the_menu_keys_open_the_diff_menu_and_esc_returns_focus(cx: &mut TestAppContext) {
    let (opened, _dirs) = walked(cx, "dk");
    cx.update(crate::screens::compare_host::bind_compare_keys);
    cx.simulate_keystrokes(opened.0, "enter");
    cx.run_until_parked();
    for key in ["shift-f10", "menu"] {
        cx.simulate_keystrokes(opened.0, key);
        cx.run_until_parked();
        assert!(menu_open(cx, &opened), "{key} opens the diff menu");
        assert!(menu_drawn(cx, &opened), "{key} draws the diff menu");
        cx.simulate_keystrokes(opened.0, "escape");
        cx.run_until_parked();
        assert!(!menu_open(cx, &opened), "Esc after {key} closes the menu");
        assert!(!menu_drawn(cx, &opened), "Esc after {key} removes the rows");
        let focused = cx
            .update_window(opened.0, |_, window, _| {
                let stack = window.context_stack();
                let has = |name| stack.iter().any(|context| context.contains(name));
                has("CompareView") && !has("ContextMenu")
            })
            .expect("window");
        assert!(
            focused,
            "Esc after {key} gives the focus back to the file diff"
        );
    }
}
