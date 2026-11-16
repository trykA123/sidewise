//! Tests of L1: the Keys sheet and the View menu mounted in the shell, opened from the topbar and
//! the keys, and the View action's availability by screen.

use std::fs;
use std::sync::Arc;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext,
};
use navo_core::Location;
use navo_ui::browser::{CompareControl, CompareHooks, CompareSink};
use navo_ui::compare_diff::CompareOptions;

use crate::compare_tab_tests::{TempDir, shell, submit};
use crate::screens::compare_tab::{CompareRequest, TabView, install_test_hooks};
use crate::shell::Shell;
use crate::start_tabs::StartChoice;

type Opened = (AnyWindowHandle, Entity<Shell>);

/// A compare engine that starts and then does nothing.
struct Quiet;

impl CompareHooks for Quiet {
    /// Every location can be compared in this fixture.
    fn supports(&self, _location: &Location) -> bool {
        true
    }

    /// Returns an idle control and drops the sink.
    fn start(
        &self,
        _sides: (&Location, &Location),
        _options: &CompareOptions,
        _sink: CompareSink,
    ) -> Box<dyn CompareControl> {
        Box::new(Idle)
    }
}

/// A compare control that ignores every request.
struct Idle;

impl CompareControl for Idle {
    /// Ignores a focus request.
    fn focus(&self, _row_ids: Vec<u64>) {}
    /// Ignores a stop request for one row.
    fn stop_row(&self, _id: u64) {}
    /// Ignores a stop request for the whole walk.
    fn stop_all(&self) {}
    /// Ignores a refresh request.
    fn refresh(&self) {}
    /// Ignores a request to resume a stopped walk.
    fn continue_walk(&self) {}
}

/// Clicks the centre of the element `id` and lets the window settle.
fn click(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) {
    let at = cx
        .update_window(window, |_, window, _| window.find(id).bounds().center())
        .expect("window");
    VisualTestContext::from_window(window, cx).simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .expect("window");
}

/// Reads the Keys sheet and View menu state: `(keys open, menu open)`.
fn open_state(cx: &mut TestAppContext, (_, view): &Opened) -> (bool, bool) {
    view.read_with(cx, |shell, cx| {
        (shell.keys_open(cx), shell.view_menu_open())
    })
}

/// The focused element's handle, to compare before and after a layer.
fn focus_id(cx: &mut TestAppContext, window: AnyWindowHandle) -> Option<gpui_kit::FocusHandle> {
    cx.update_window(window, |_, window, cx| window.focused(cx))
        .expect("window")
}

/// A shell on a compare tab of two folders (quiet engine) or two files (opens the File view).
fn on_compare(cx: &mut TestAppContext, tag: &str, files: bool) -> (Opened, [TempDir; 2]) {
    let dirs = [
        TempDir::new(&format!("{tag}-l")),
        TempDir::new(&format!("{tag}-r")),
    ];
    let opened = shell(cx);
    cx.update(|cx| install_test_hooks(Arc::new(Quiet), cx));
    if files {
        let (left, right) = (dirs[0].path().join("a.txt"), dirs[1].path().join("a.txt"));
        fs::write(&left, "one").expect("write");
        fs::write(&right, "two").expect("write");
        let request = CompareRequest {
            left,
            right,
            choice: StartChoice::default(),
        };
        let (window, view) = &opened;
        cx.update_window(*window, |_, window, cx| {
            view.update(cx, |shell, cx| shell.open_compare((0, request), window, cx))
        })
        .expect("window")
        .expect("open");
    } else {
        submit(cx, &opened, [dirs[0].path(), dirs[1].path()]);
    }
    cx.run_until_parked();
    (opened, dirs)
}

#[gpui_kit::test]
fn on_start_the_keys_action_and_the_key_open_the_sheet_and_escape_closes_it(
    cx: &mut TestAppContext,
) {
    let opened = shell(cx);
    let before = focus_id(cx, opened.0);
    assert!(before.is_some(), "start focuses a field");
    click(cx, opened.0, "top-keys");
    assert_eq!(open_state(cx, &opened), (true, false), "Keys pill opens it");
    cx.simulate_keystrokes(opened.0, "escape");
    assert_eq!(open_state(cx, &opened), (false, false), "Escape closes it");
    assert_eq!(focus_id(cx, opened.0), before, "focus returns");
    cx.simulate_keystrokes(opened.0, "f1");
    assert_eq!(open_state(cx, &opened), (true, false), "F1 opens it");
    cx.simulate_keystrokes(opened.0, "escape");
    assert_eq!(open_state(cx, &opened), (false, false));
    assert_eq!(focus_id(cx, opened.0), before, "focus returns again");
}

#[gpui_kit::test]
fn on_start_the_view_action_opens_the_menu_and_escape_closes_it(cx: &mut TestAppContext) {
    let opened = shell(cx);
    let before = focus_id(cx, opened.0);
    click(cx, opened.0, "top-view");
    assert_eq!(open_state(cx, &opened), (false, true), "View pill opens it");
    cx.simulate_keystrokes(opened.0, "escape");
    assert_eq!(open_state(cx, &opened), (false, false), "Escape closes it");
    assert_eq!(focus_id(cx, opened.0), before, "focus returns");
    cx.simulate_keystrokes(opened.0, "alt-v");
    assert_eq!(open_state(cx, &opened), (false, true), "Alt+V opens it");
}

#[gpui_kit::test]
fn the_keys_sheet_opens_on_a_compare_tab_too(cx: &mut TestAppContext) {
    let (opened, _dirs) = on_compare(cx, "keys", false);
    cx.simulate_keystrokes(opened.0, "f1");
    assert_eq!(open_state(cx, &opened), (true, false));
    cx.simulate_keystrokes(opened.0, "escape");
    assert_eq!(open_state(cx, &opened), (false, false));
}

#[gpui_kit::test]
fn view_is_available_on_folders_and_not_on_file(cx: &mut TestAppContext) {
    let (folders, _dirs) = on_compare(cx, "folders", false);
    let tab = folders.1.read_with(cx, |s, _| s.active_compare().cloned());
    assert_eq!(
        tab.expect("tab").read_with(cx, |t, _| t.view()),
        TabView::Folders
    );
    assert!(folders.1.read_with(cx, |s, cx| s.view_action_available(cx)));
    cx.simulate_keystrokes(folders.0, "alt-v");
    assert_eq!(open_state(cx, &folders), (false, true), "opens on Folders");

    let (file, _more) = on_compare(cx, "file", true);
    let tab = file.1.read_with(cx, |s, _| s.active_compare().cloned());
    assert_eq!(
        tab.expect("tab").read_with(cx, |t, _| t.view()),
        TabView::File
    );
    assert!(!file.1.read_with(cx, |s, cx| s.view_action_available(cx)));
    cx.simulate_keystrokes(file.0, "alt-v");
    assert_eq!(
        open_state(cx, &file),
        (false, false),
        "Navo's Alt+V, not ours"
    );
}
