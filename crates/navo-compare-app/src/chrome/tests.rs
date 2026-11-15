//! Tests of the topbar (NC66): every pill and action key fires the registered callback, and
//! the bar fits the 780 px minimum width without clipping or losing its side insets.

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::base::test_support::{find, snapshots};
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, ElementId, Entity, Modifiers, Point, Role,
    SharedString, TestAppContext, VisualTestContext, WindowBounds, WindowOptions, point, px, size,
};
use navo_compare_profile::appearance::PaletteId;
use navo_ui::tokens::Mode;

use super::topbar_status::{Action, Crumbs, MIN_W, Topbar, bind_keys};
use crate::theme::apply_sidewise;
use crate::theme_data::palettes::DIMS;

/// The element ids of the search pill and the five action pills, in bar order.
const IDS: [(&str, Action); 6] = [
    ("top-search", Action::Search),
    ("top-theme", Action::Theme),
    ("top-accounts", Action::Accounts),
    ("top-view", Action::View),
    ("top-keys", Action::Keys),
    ("top-sync", Action::Sync),
];

/// Opens a focused, drawn topbar in a Light-themed window of `width`, recording every action fired.
fn topbar(
    cx: &mut TestAppContext,
    width: f32,
) -> (AnyWindowHandle, Entity<Topbar>, Rc<RefCell<Vec<Action>>>) {
    topbar_in(cx, width, Mode::Light)
}

/// Opens a focused, drawn topbar in `mode` at `width`, recording every action fired.
fn topbar_in(
    cx: &mut TestAppContext,
    width: f32,
    mode: Mode,
) -> (AnyWindowHandle, Entity<Topbar>, Rc<RefCell<Vec<Action>>>) {
    let fired = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        gpui_kit::init(cx);
        apply_sidewise(PaletteId::Violet, mode, cx);
        bind_keys(cx);
    });
    let log = Rc::clone(&fired);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Point::default(),
            size: size(px(width), px(600.)),
        })),
        ..Default::default()
    };
    let (window, view) = cx.update(|cx| {
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| {
                Topbar::new(
                    Crumbs {
                        title: "New comparison".into(),
                        detail: "Folder · File · Git".into(),
                    },
                    move |action, _| log.borrow_mut().push(action),
                    window,
                    cx,
                )
            })
        })
        .expect("open test window")
    });
    cx.update_window(window, |_, window, cx| {
        let handle = view.read(cx).focus_handle();
        handle.focus(window, cx);
        window.render_frame(cx);
    })
    .expect("window");
    (window, view, fired)
}

/// Clicks the centre of one observed element with a real mouse down and up.
fn click(cx: &mut TestAppContext, window: AnyWindowHandle, id: &str) {
    let at = cx
        .update_window(window, |_, window, _| {
            let found = find(window, &[], &ElementId::Name(id.into()))
                .unwrap_or_else(|| panic!("no {id} element in the drawn topbar"));
            let b = found.bounds();
            point(
                b.origin.x + b.size.width / 2.0,
                b.origin.y + b.size.height / 2.0,
            )
        })
        .expect("window");
    VisualTestContext::from_window(window, cx).simulate_click(at, Modifiers::none());
}

/// Clicking each pill fires its own registered action, once.
#[gpui_kit::test]
fn every_pill_fires_its_registered_action(cx: &mut TestAppContext) {
    let (window, _view, fired) = topbar(cx, 1024.);
    for (id, action) in IDS {
        click(cx, window, id);
        assert_eq!(
            fired.borrow().last(),
            Some(&action),
            "clicking {id} must fire {action:?}"
        );
    }
    assert_eq!(fired.borrow().len(), IDS.len(), "one callback per click");
}

/// Alt+V, Alt+S and / fire the same callbacks as their pills.
#[gpui_kit::test]
fn the_action_keys_fire_the_same_callbacks(cx: &mut TestAppContext) {
    let (window, _view, fired) = topbar(cx, 1024.);
    for (keys, action) in [
        ("alt-v", Action::View),
        ("alt-s", Action::Sync),
        ("/", Action::Search),
    ] {
        cx.simulate_keystrokes(window, keys);
        assert_eq!(
            fired.borrow().last(),
            Some(&action),
            "{keys} must fire {action:?}"
        );
    }
}

/// At the minimum width no visible part of the bar overflows the window either side.
#[gpui_kit::test]
fn the_bar_fits_the_minimum_width_without_clipping(cx: &mut TestAppContext) {
    let (window, _view, _fired) = topbar(cx, MIN_W);
    cx.update_window(window, |_, window, _| {
        let viewport = window.viewport_size();
        let mut checked = 0;
        for snap in snapshots(window) {
            let b = snap.bounds();
            if !snap.visible() || b.size.width <= px(0.) {
                continue;
            }
            assert!(
                b.origin.x >= px(0.),
                "{:?} starts left of the window at {MIN_W}px",
                snap.path()
            );
            assert!(
                b.origin.x + b.size.width <= viewport.width,
                "{:?} overflows the right edge at {MIN_W}px",
                snap.path()
            );
            checked += 1;
        }
        assert!(checked >= IDS.len(), "the pills must all be drawn");
    })
    .expect("window");
}

/// The bar keeps the owner's 16 px side inset for its text at the minimum width.
#[gpui_kit::test]
fn the_text_keeps_the_side_insets(cx: &mut TestAppContext) {
    let (window, _view, _fired) = topbar(cx, MIN_W);
    cx.update_window(window, |_, window, _| {
        let viewport = window.viewport_size();
        let first = find(window, &[], &ElementId::Name("top-search".into()))
            .expect("search pill")
            .bounds();
        let last = find(window, &[], &ElementId::Name("top-sync".into()))
            .expect("sync pill")
            .bounds();
        assert!(
            first.origin.x >= DIMS.inset_x,
            "the bar's content must start at least 16px in"
        );
        assert!(
            last.origin.x + last.size.width <= viewport.width - DIMS.inset_x,
            "the bar's content must end at least 16px before the edge"
        );
    })
    .expect("window");
}

/// The filter text round-trips through `set_filter`, `filter()` and the rendered text field.
#[gpui_kit::test]
fn the_filter_text_round_trips(cx: &mut TestAppContext) {
    let (window, view, _fired) = topbar(cx, 1024.);
    cx.update_window(window, |_, window, cx| {
        assert_eq!(
            view.read(cx).filter(),
            &SharedString::default(),
            "starts empty"
        );
        view.update(cx, |bar, cx| bar.set_filter("main.rs", window, cx));
        window.render_frame(cx);
        assert_eq!(view.read(cx).filter(), &SharedString::from("main.rs"));
        assert_eq!(window.find("top-search").value(), Some("main.rs"));
    })
    .expect("window");
}

/// Pressing Tab reaches Accounts without mouse input, and Enter/Space activates it.
#[gpui_kit::test]
fn tab_reaches_accounts_and_activates_without_mouse(cx: &mut TestAppContext) {
    let (window, _view, fired) = topbar(cx, 1024.);
    cx.update_window(window, |_, window, cx| {
        let initial = window.find("top-accounts");
        assert_eq!(
            initial.role(),
            Some(Role::Button),
            "Accounts control must expose Role::Button"
        );
        let accounts = window
            .try_find_by_label("Accounts")
            .expect("Accounts control must expose accessible label \"Accounts\"");
        assert_eq!(accounts.focused(), Some(false), "Accounts starts unfocused");

        let mut reached = false;
        for _ in 0..=IDS.len() {
            window.press("tab", cx);
            if window.find_by_label("Accounts").focused() == Some(true) {
                reached = true;
                break;
            }
        }
        assert!(reached, "Tab must reach Accounts without a mouse");

        window.press("enter", cx);
        window.press("space", cx);
    })
    .expect("window");

    assert_eq!(
        fired.borrow().as_slice(),
        [Action::Accounts, Action::Accounts],
        "Enter and Space on focused Accounts must each fire Action::Accounts once"
    );
}

/// Verifies `top-search` owns the labeled text input and receives Tab focus.
fn assert_search_tab_focus(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    window.press("tab", cx);
    let search_by_id = window.find("top-search");
    let search_by_label = window.find_by_label("Search files");
    assert!(
        search_by_label.path().starts_with(search_by_id.path()),
        "top-search must own the Search files input"
    );
    assert_eq!(search_by_label.role(), Some(Role::TextInput));
    assert_eq!(search_by_label.focused(), Some(true));
}

/// Search has text-input semantics, and the five action pills have button semantics, accessible
/// labels, Tab/Shift-Tab focus, and Enter/Space keyboard activation.
#[gpui_kit::test]
fn every_control_has_button_semantics_tab_focus_and_keyboard_activation(cx: &mut TestAppContext) {
    const BUTTONS: [(&str, &str, Action); 5] = [
        ("top-theme", "Theme", Action::Theme),
        ("top-accounts", "Accounts", Action::Accounts),
        ("top-view", "View", Action::View),
        ("top-keys", "Keys", Action::Keys),
        ("top-sync", "Sync", Action::Sync),
    ];

    let (window, _view, fired) = topbar(cx, 1024.);
    cx.update_window(window, |_, window, cx| {
        assert_search_tab_focus(window, cx);
        for (idx, (id, label, action)) in BUTTONS.iter().enumerate() {
            window.press("tab", cx);
            let by_label = window.find_by_label(label);
            assert_eq!(window.find(*id).path(), by_label.path(), "{id} mismatch");
            assert_eq!(by_label.role(), Some(Role::Button), "{id} role");
            assert_eq!(by_label.focused(), Some(true), "Tab #{} {id}", idx + 1);
            let key = if idx % 2 == 0 { "enter" } else { "space" };
            window.press(key, cx);
            assert_eq!(fired.borrow().last(), Some(action), "{key} on {id}");
        }
        window.press("shift-tab", cx);
        assert_eq!(window.find_by_label("Keys").focused(), Some(true));
        window.press("space", cx);
    })
    .expect("window");

    assert_eq!(fired.borrow().len(), BUTTONS.len() + 1);
    assert_eq!(fired.borrow().last(), Some(&Action::Keys));
}

/// Shortcuts still fire when a pill holds focus, and Escape does not fire stale actions or trap focus.
#[gpui_kit::test]
fn shortcuts_work_from_focused_button_and_escape_does_not_trap(cx: &mut TestAppContext) {
    let (window, _view, fired) = topbar(cx, 1024.);
    cx.update_window(window, |_, window, cx| {
        window.press("tab", cx);
        window.press("tab", cx);
        assert_eq!(window.find_by_label("Theme").focused(), Some(true));

        window.press("escape", cx);
        assert!(
            fired.borrow().is_empty(),
            "Escape on a resting topbar button must not fire any action"
        );
        window.press("tab", cx);
        assert_eq!(
            window.find_by_label("Accounts").focused(),
            Some(true),
            "focus must still advance after Escape"
        );
    })
    .expect("window");

    for (keys, action) in [
        ("alt-v", Action::View),
        ("alt-s", Action::Sync),
        ("/", Action::Search),
    ] {
        cx.simulate_keystrokes(window, keys);
        assert_eq!(
            fired.borrow().last(),
            Some(&action),
            "{keys} must still fire {action:?} when a pill is focused"
        );
    }
}

/// In Dark mode at the 780 px minimum width, Tab reaches Accounts and Enter activates it without clipping.
#[gpui_kit::test]
fn dark_mode_minimum_width_reaches_accounts_by_keyboard(cx: &mut TestAppContext) {
    let (window, _view, fired) = topbar(cx, MIN_W);
    cx.update_window(window, |_, window, cx| {
        apply_sidewise(PaletteId::Violet, Mode::Dark, cx);
        window.render_frame(cx);
        for _ in 0..3 {
            window.press("tab", cx);
        }
        let accounts = window.find_by_label("Accounts");
        assert_eq!(accounts.focused(), Some(true));
        assert!(
            accounts.visible(),
            "Accounts must stay visible at {MIN_W}px"
        );
        window.press("enter", cx);
    })
    .expect("window");

    assert_eq!(fired.borrow().as_slice(), [Action::Accounts]);
}

/// Asserts that the visible TextField value, `Topbar::filter`, and change callback all agree.
fn assert_filter_synced(
    window: &gpui_kit::Window,
    bar: &Topbar,
    changes: &[SharedString],
    expected: &str,
) {
    assert_eq!(window.find("top-search").value(), Some(expected));
    assert_eq!(bar.filter().as_str(), expected);
    assert_eq!(changes.last().map(SharedString::as_str), Some(expected));
}

/// Pressing `/` focuses the inline filter input, real typing and Ctrl+A/Backspace clearing update
/// both `Topbar::filter` and the registered change callback, and Tab still reaches Accounts.
#[gpui_kit::test]
fn slash_focuses_filter_real_typing_and_clearing_sync_callback_and_tab_navigates(
    cx: &mut TestAppContext,
) {
    let (window, view, fired) = topbar(cx, 1024.);
    let changes = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let sink = Rc::clone(&changes);
    cx.update(|cx| {
        view.update(cx, |bar, _| {
            bar.set_on_filter_change(move |text, _| sink.borrow_mut().push(text));
        });
    });

    cx.simulate_keystrokes(window, "/");
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(fired.borrow().as_slice(), [Action::Search]);
        assert_eq!(window.find("top-search").focused(), Some(true));
        window.input("foo.rs", cx);
        window.render_frame(cx);
    })
    .expect("window");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        assert_filter_synced(window, view.read(cx), &changes.borrow(), "foo.rs");
        window.press("ctrl-a", cx);
        window.press("backspace", cx);
        window.render_frame(cx);
    })
    .expect("window");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        assert_filter_synced(window, view.read(cx), &changes.borrow(), "");
        window.press("tab", cx);
        window.press("tab", cx);
        assert_eq!(window.find_by_label("Accounts").focused(), Some(true));
        window.press("enter", cx);
    })
    .expect("window");

    assert_eq!(
        fired.borrow().as_slice(),
        [Action::Search, Action::Accounts]
    );
}

/// At the 780 px minimum width in both Light and Dark modes, typing a long path into the inline
/// filter stays clipped cleanly inside the bar while keeping action buttons visible and reachable.
#[gpui_kit::test]
fn editable_filter_fits_light_and_dark_minimum_width_without_clipping(cx: &mut TestAppContext) {
    for mode in [Mode::Light, Mode::Dark] {
        let (window, view, _fired) = topbar_in(cx, MIN_W, mode);
        cx.simulate_keystrokes(window, "/");
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.input("crates/navo-compare-app/src/chrome/topbar_status.rs", cx);
            window.render_frame(cx);
        })
        .expect("window");
        cx.run_until_parked();

        cx.update_window(window, |_, window, cx| {
            assert_eq!(
                view.read(cx).filter(),
                &SharedString::from("crates/navo-compare-app/src/chrome/topbar_status.rs"),
                "{mode:?}: long filter must update Topbar::filter"
            );

            let viewport = window.viewport_size();
            let search = find(window, &[], &ElementId::Name("top-search".into()))
                .expect("top-search")
                .bounds();
            let sync = find(window, &[], &ElementId::Name("top-sync".into()))
                .expect("top-sync")
                .bounds();
            assert!(
                search.origin.x >= DIMS.inset_x && search.size.width > px(0.),
                "{mode:?}: editable filter must keep side inset and non-zero width"
            );
            assert!(
                sync.origin.x + sync.size.width <= viewport.width - DIMS.inset_x,
                "{mode:?}: right-most pill must stay within {MIN_W}px inset"
            );

            window.press("tab", cx);
            window.press("tab", cx);
            assert_eq!(
                window.find_by_label("Accounts").focused(),
                Some(true),
                "{mode:?}: Tab from filter must reach Accounts at {MIN_W}px"
            );
        })
        .expect("window");
    }
}
