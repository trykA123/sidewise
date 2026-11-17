//! Tests of the Folders context menu: right click, copy paths, expand all and the shortcut text.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent,
    TestAppContext, VisualTestContext,
};

use super::tests::{shown, view_with};
use super::{FoldersView, bind_keys};
use crate::menu_host::{bind_menu_keys, menu_view};

/// A focused view over the fixture with roots `/left` and `/right`, menu keys bound.
fn setup(cx: &mut TestAppContext) -> (AnyWindowHandle, gpui_kit::Entity<FoldersView>) {
    let roots = [abs_root("left"), abs_root("right")];
    let (window, view, _) = view_with(cx, move |cx| FoldersView::new(cx).with_roots(roots));
    cx.update(|cx| {
        bind_keys(cx);
        bind_menu_keys(cx);
    });
    (window, view)
}

/// Presses and releases the right button on the centre of line `index`.
fn right_click(cx: &mut TestAppContext, window: AnyWindowHandle, index: usize) {
    let at = cx
        .update_window(window, |_, w, _| {
            w.find(("folder-line", index)).bounds().center()
        })
        .expect("window");
    let mut vcx = VisualTestContext::from_window(window, cx);
    vcx.simulate_event(MouseDownEvent {
        button: MouseButton::Right,
        position: at,
        modifiers: Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    vcx.simulate_event(MouseUpEvent {
        button: MouseButton::Right,
        position: at,
        modifiers: Modifiers::default(),
        click_count: 1,
    });
}

/// The command lines of the open menu: (text, accelerator, enabled); separators are skipped.
fn menu_lines(
    cx: &mut TestAppContext,
    view: &gpui_kit::Entity<FoldersView>,
) -> Vec<(String, Option<String>, bool)> {
    let menu = view.read_with(cx, |view, _| menu_view(&view.menu));
    let menu = menu.expect("open menu");
    menu.read_with(cx, |menu, _| {
        let sep = sidewise_core::shell_menu::ItemKind::Separator;
        let items = menu.menu().items.iter().filter(|item| item.kind != sep);
        items
            .map(|i| (i.text.clone(), i.accelerator.clone(), i.enabled))
            .collect()
    })
}

/// Expands `a/b`, then right-clicks `a/b/y.txt` (only on the left) from another cursor line.
fn right_click_the_left_only_file(
    cx: &mut TestAppContext,
) -> (AnyWindowHandle, gpui_kit::Entity<FoldersView>) {
    let (window, view) = setup(cx);
    cx.update(|cx| {
        view.update(cx, |this, cx| {
            this.select_path("a/b/y.txt", cx);
            this.select_path("a", cx);
        })
    });
    cx.update_window(window, |_, window, _| window.refresh())
        .expect("window");
    cx.run_until_parked();
    assert_eq!(shown(&view, cx)[3], "a/b/y.txt");
    right_click(cx, window, 3);
    cx.run_until_parked();
    (window, view)
}

#[gpui_kit::test]
fn right_click_selects_and_shows_the_file_menu(cx: &mut TestAppContext) {
    let (_, view) = right_click_the_left_only_file(cx);
    cx.update(|cx| assert_eq!(view.read(cx).selected_path().as_deref(), Some("a/b/y.txt")));
    let lines = menu_lines(cx, &view);
    let labels: Vec<_> = lines.iter().map(|l| l.0.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Open diff",
            "Open in Stream",
            "Copy left path",
            "Copy right path",
            "Copy name",
            "Reveal left in Explorer",
            "Reveal right in Explorer"
        ]
    );
    let enabled: Vec<_> = lines.iter().map(|l| l.2).collect();
    let left = cfg!(windows);
    assert_eq!(enabled, [true, true, true, false, true, left, false]);
}

#[gpui_kit::test]
fn the_menu_shows_shift_enter_on_open_in_stream(cx: &mut TestAppContext) {
    let (_, view) = right_click_the_left_only_file(cx);
    let lines = menu_lines(cx, &view);
    assert_eq!(lines[0].1.as_deref(), Some("Enter"));
    assert_eq!(lines[1].1.as_deref(), Some("Shift+Enter"));
    assert_eq!(lines[2].1.as_deref(), Some("Ctrl+Shift+C"));
}

#[gpui_kit::test]
fn copy_left_path_writes_the_absolute_path(cx: &mut TestAppContext) {
    let (window, view) = setup(cx);
    cx.update(|cx| view.update(cx, |this, cx| this.select_path("a/x.txt", cx)));
    cx.simulate_keystrokes(window, "ctrl-shift-c");
    let text = cx.read_from_clipboard().and_then(|item| item.text());
    let want = abs_root("left").join("a").join("x.txt");
    assert_eq!(text.as_deref(), want.to_str());
}

#[gpui_kit::test]
fn shift_right_expands_all_below(cx: &mut TestAppContext) {
    let (window, view) = setup(cx);
    cx.simulate_keystrokes(window, "shift-right");
    assert_eq!(
        shown(&view, cx),
        ["a", "a/x.txt", "a/b", "a/b/y.txt", "c", "z.txt"]
    );
}

#[gpui_kit::test]
fn shift_f10_opens_the_menu_on_the_cursor_line(cx: &mut TestAppContext) {
    let (window, view) = setup(cx);
    cx.simulate_keystrokes(window, "shift-f10");
    cx.run_until_parked();
    let lines = menu_lines(cx, &view);
    assert_eq!(lines[0].0, "Expand");
    assert_eq!(lines[1].0, "Expand all");
}

/// An absolute test root: `/name` on Unix, `C:\name` on Windows, where a path without a drive
/// is not absolute and the app shows it drive-qualified.
fn abs_root(name: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(format!("C:\\{name}"))
    } else {
        PathBuf::from(format!("/{name}"))
    }
}
