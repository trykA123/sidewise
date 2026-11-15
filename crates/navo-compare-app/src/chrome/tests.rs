//! Tests of the topbar (NC66): every pill and action key fires the registered callback, and
//! the bar fits the 780 px minimum width without clipping or losing its side insets.

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::base::test_support::{find, snapshots};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, ElementId, Entity, Modifiers, Point, SharedString,
    TestAppContext, VisualTestContext, WindowBounds, WindowOptions, point, px, size,
};
use navo_compare_profile::appearance::PaletteId;
use navo_ui::tokens::Mode;

use super::topbar_status::{Action, MIN_W, Topbar, bind_keys};
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

/// Opens a focused, drawn topbar in a themed window of `width`, recording every action fired.
fn topbar(
    cx: &mut TestAppContext,
    width: f32,
) -> (AnyWindowHandle, Entity<Topbar>, Rc<RefCell<Vec<Action>>>) {
    let fired = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        gpui_kit::init(cx);
        apply_sidewise(PaletteId::Violet, Mode::Light, cx);
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
        gpui_kit::open_window(options, cx, |_, cx| {
            cx.new(|cx| {
                Topbar::new(
                    "New comparison",
                    "Folder · File · Git",
                    move |action, _| log.borrow_mut().push(action),
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

/// The filter text is readable and replaceable.
#[gpui_kit::test]
fn the_filter_text_round_trips(cx: &mut TestAppContext) {
    let (_window, view, _fired) = topbar(cx, 1024.);
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).filter(),
            &SharedString::default(),
            "starts empty"
        );
        view.update(cx, |bar, cx| bar.set_filter("main.rs", cx));
        assert_eq!(view.read(cx).filter(), &SharedString::from("main.rs"));
    });
}
