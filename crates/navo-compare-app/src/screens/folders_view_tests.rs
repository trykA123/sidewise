//! Tests of `FoldersView`: cursor keys, Enter on a file, and the summary chips.

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, Focusable as _, TestAppContext};
use navo_compare_profile::appearance::PaletteId;
use navo_ui::browser::{CompareKind, CompareRow, CompareSide, RowStatus};
use navo_ui::tokens::Mode;

use navo_ui::compare_diff::map::MapEvent;

use super::{FoldersEvent, FoldersView, bind_keys};
use crate::test_window::open;
use crate::theme::apply_sidewise;

/// A side of `kind` with a placeholder size.
fn side(kind: CompareKind) -> Option<CompareSide> {
    Some(CompareSide {
        kind,
        size: 1,
        modified_ms: None,
    })
}

/// A row present on both sides.
fn row(id: u64, path: &str, status: RowStatus, kind: CompareKind) -> CompareRow {
    CompareRow::new(id, path, status, (side(kind), side(kind)))
}

/// The U17a' fixture: folders `a`, `a/b`, `c`, with files under them and `z.txt`.
fn rows() -> Vec<CompareRow> {
    use CompareKind::{File, Folder};
    let only_left = CompareRow::new(4, "a/b/y.txt", RowStatus::OnlyLeft, (side(File), None));
    vec![
        row(1, "a", RowStatus::Different, Folder),
        row(2, "a/x.txt", RowStatus::Different, File),
        row(3, "a/b", RowStatus::Different, Folder),
        only_left,
        row(5, "c", RowStatus::Same, Folder),
        row(6, "c/s.txt", RowStatus::Same, File),
        row(7, "z.txt", RowStatus::Same, File),
    ]
}

/// A focused view over the fixture, with the events it emits collected.
fn view(
    cx: &mut TestAppContext,
) -> (
    AnyWindowHandle,
    Entity<FoldersView>,
    Rc<RefCell<Vec<FoldersEvent>>>,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        apply_sidewise(PaletteId::Violet, Mode::Light, cx);
        bind_keys(cx);
    });
    let (window, view) = open(cx, |_, cx| FoldersView::new(cx));
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();
    cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &FoldersEvent, _| {
            seen.borrow_mut().push(*event)
        })
        .detach();
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |this, cx| this.set_rows(rows(), cx));
        view.read(cx).focus_handle(cx).focus(window, cx);
        window.render_frame(cx);
    })
    .expect("window");
    (window, view, events)
}

/// The paths of the lines shown now.
fn shown(view: &Entity<FoldersView>, cx: &mut TestAppContext) -> Vec<String> {
    cx.update(|cx| {
        view.read(cx)
            .tree
            .lines()
            .into_iter()
            .map(|line| line.path)
            .collect()
    })
}

/// Right on a closed folder opens it; right again moves to its first child; left goes back up.
#[gpui_kit::test]
fn right_opens_a_folder_then_enters_it(cx: &mut TestAppContext) {
    let (window, view, _) = view(cx);
    assert_eq!(shown(&view, cx), ["a", "c", "z.txt"]);
    cx.simulate_keystrokes(window, "right");
    assert_eq!(shown(&view, cx), ["a", "a/x.txt", "a/b", "c", "z.txt"]);
    cx.simulate_keystrokes(window, "right");
    cx.update(|cx| assert_eq!(view.read(cx).selected_path().as_deref(), Some("a/x.txt")));
    cx.simulate_keystrokes(window, "left");
    cx.update(|cx| assert_eq!(view.read(cx).selected_path().as_deref(), Some("a")));
    cx.simulate_keystrokes(window, "left");
    assert_eq!(shown(&view, cx), ["a", "c", "z.txt"]);
}

/// Enter on `a/x.txt` emits `OpenPair(2)`; Enter on a folder emits nothing.
#[gpui_kit::test]
fn enter_on_a_file_opens_its_pair(cx: &mut TestAppContext) {
    let (window, view, events) = view(cx);
    cx.simulate_keystrokes(window, "enter");
    assert!(events.borrow().is_empty(), "a folder only toggles");
    assert_eq!(shown(&view, cx).len(), 5);
    cx.simulate_keystrokes(window, "down");
    cx.update(|cx| assert_eq!(view.read(cx).selected_path().as_deref(), Some("a/x.txt")));
    cx.simulate_keystrokes(window, "enter");
    assert_eq!(*events.borrow(), [FoldersEvent::OpenPair(2)]);
}

/// With `a` open, a click on the Differences chip leaves `a`, `a/x.txt` and `a/b`.
#[gpui_kit::test]
fn differences_chip_filters_the_tree(cx: &mut TestAppContext) {
    let (window, view, _) = view(cx);
    cx.simulate_keystrokes(window, "right");
    cx.update_window(window, |_, window, cx| window.click("chip-Differences", cx))
        .expect("window");
    assert_eq!(shown(&view, cx), ["a", "a/x.txt", "a/b"]);
}

/// `select_path` opens the ancestors and puts the cursor on the file.
#[gpui_kit::test]
fn select_path_opens_ancestors(cx: &mut TestAppContext) {
    let (_, view, _) = view(cx);
    let found = cx.update(|cx| view.update(cx, |this, cx| this.select_path("a/b/y.txt", cx)));
    assert!(found);
    assert_eq!(
        shown(&view, cx),
        ["a", "a/x.txt", "a/b", "a/b/y.txt", "c", "z.txt"]
    );
    cx.update(|cx| assert_eq!(view.read(cx).selected_path().as_deref(), Some("a/b/y.txt")));
    assert!(!cx.update(|cx| view.update(cx, |this, cx| this.select_path("nope", cx))));
}

/// With `a` open, a `Jump(2)` from the change map puts the cursor on `a/b`.
#[gpui_kit::test]
fn map_jump_moves_the_cursor(cx: &mut TestAppContext) {
    let (window, view, _) = view(cx);
    cx.simulate_keystrokes(window, "right");
    let map = cx.update(|cx| view.read(cx).map.clone());
    cx.update(|cx| map.update(cx, |_, cx| cx.emit(MapEvent::Jump(2))));
    cx.update(|cx| assert_eq!(view.read(cx).selected_path().as_deref(), Some("a/b")));
}
