//! Tests of `StatusBar`: `Role::Status` root, full accessible summary text under truncation,
//! `Kbd` hint parsing and absence of focus stops, and Light/Dark layout across 500 and 1200 px.

use gpui_kit::base::test_support::{find, snapshots};
use gpui_kit::component::button::Button;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, ElementId, FocusHandle, Point, Render, Role,
    SharedString, TestAppContext, WindowBounds, WindowOptions, div, px, size,
};
use navo_compare_profile::appearance::PaletteId;
use sidewise_kit::prelude::*;
use sidewise_kit::primitives::Kbd;
use sidewise_kit::tokens::Mode;

use super::status_bar::{StatusBar, StatusHint};
use crate::theme::apply_sidewise;
use crate::theme_data::palettes::DIMS;

const LONG_SUMMARY: &str = "Comparing crates/navo-compare-app/src/chrome/topbar_status.rs against upstream/crates/navo-compare-app/src/chrome/status_bar.rs · 128 differences · 41 orphans · 912 identical files";

/// Builds a caller-owned [`StatusHint`] for tests.
fn hint(id: &'static str, keys: &'static str, label: &'static str) -> StatusHint {
    StatusHint {
        id: id.into(),
        keys: keys.into(),
        label: label.into(),
    }
}

/// Two existing Shell hints shown by default on the right of the status bar.
fn shell_hints() -> Vec<StatusHint> {
    vec![
        hint("hint-new", "ctrl-t", "New tab"),
        hint("hint-close", "ctrl-w", "Close tab"),
    ]
}

/// Minimal window host pairing two focusable buttons around one `StatusBar`.
struct Host {
    summary: SharedString,
    hints: Vec<StatusHint>,
    handle: FocusHandle,
}

impl Render for Host {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .tab_group()
            .track_focus(&self.handle)
            .flex()
            .flex_col()
            .size_full()
            .child(Button::new("before-status").accessibility_label("Before"))
            .child(div().flex_1().min_h_0())
            .child(StatusBar::new(self.summary.clone(), self.hints.clone()))
            .child(Button::new("after-status").accessibility_label("After"))
    }
}

/// Opens a themed window of `width` drawing `StatusBar` in `mode`.
fn open_bar(
    cx: &mut TestAppContext,
    width: f32,
    mode: Mode,
    (summary, hints): (&str, Vec<StatusHint>),
) -> AnyWindowHandle {
    cx.update(|cx| {
        gpui_kit::init(cx);
        apply_sidewise(PaletteId::Violet, mode, cx);
    });
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Point::default(),
            size: size(px(width), px(300.)),
        })),
        ..Default::default()
    };
    let summary: SharedString = summary.to_owned().into();
    let (window, view) = cx
        .update(|cx| {
            gpui_kit::open_window(options, cx, |_, cx| {
                cx.new(|cx| Host {
                    summary,
                    hints,
                    handle: cx.focus_handle(),
                })
            })
        })
        .expect("open test window");
    cx.update_window(window, |_, window, cx| {
        let handle = view.read(cx).handle.clone();
        handle.focus(window, cx);
        window.render_frame(cx);
    })
    .expect("window");
    window
}

/// Root exposes `Role::Status`, summary keeps full accessible text, and only valid hints render.
#[gpui_kit::test]
fn root_has_status_role_summary_keeps_full_text_and_invalid_hint_is_omitted(
    cx: &mut TestAppContext,
) {
    let mut hints = shell_hints();
    hints.push(hint("hint-bad", "not-a-valid-key", "Ignored"));
    let window = open_bar(cx, 1200., Mode::Light, ("3 differences · 1 orphan", hints));

    cx.update_window(window, |_, window, _| {
        let root = window.find("status-bar");
        assert_eq!(root.role(), Some(Role::Status));
        assert!(root.visible());

        let summary = window.find("status-summary");
        assert!(summary.path().starts_with(root.path()));
        assert_eq!(summary.role(), Some(Role::Label));
        assert_eq!(summary.label(), Some("3 differences · 1 orphan"));

        for (id, keys) in [("hint-new", "ctrl-t"), ("hint-close", "ctrl-w")] {
            let expected = Kbd::new(id, keys).expect("valid shell hint key").text();
            let kbd = window.find(id);
            assert!(kbd.path().starts_with(root.path()));
            assert_eq!(kbd.role(), Some(Role::Label));
            assert_eq!(kbd.label(), Some(expected.as_str()));
        }
        assert!(
            window.try_find("hint-bad").is_none(),
            "unparseable key hints must be omitted per Kbd::new"
        );
    })
    .expect("window");
}

/// Long summaries stay clipped inside 500 px and 1200 px bars across Light and Dark without
/// overflowing horizontally or losing their full accessible text.
#[gpui_kit::test]
fn long_summary_fits_500_and_1200_light_and_dark_preserving_accessible_text(
    cx: &mut TestAppContext,
) {
    for mode in [Mode::Light, Mode::Dark] {
        for width in [500., 1200.] {
            let window = open_bar(cx, width, mode, (LONG_SUMMARY, shell_hints()));
            cx.update_window(window, |_, window, _| {
                let viewport = window.viewport_size();
                let summary = find(window, &[], &ElementId::Name("status-summary".into()))
                    .expect("status-summary");
                let hints = find(window, &[], &ElementId::Name("status-hints".into()))
                    .expect("status-hints");
                assert_eq!(
                    summary.label(),
                    Some(LONG_SUMMARY),
                    "{mode:?} @{width}px must preserve full accessible summary text"
                );
                let sb = summary.bounds();
                let hb = hints.bounds();
                assert!(
                    sb.origin.x >= DIMS.inset_x,
                    "{mode:?} @{width}px left inset"
                );
                assert!(
                    sb.origin.x + sb.size.width <= hb.origin.x,
                    "{mode:?} @{width}px summary must not overlap hints"
                );
                assert!(
                    hb.origin.x + hb.size.width <= viewport.width - DIMS.inset_x,
                    "{mode:?} @{width}px hints must respect right inset"
                );
                for snap in snapshots(window) {
                    let b = snap.bounds();
                    if !snap.visible() || b.size.width <= px(0.) {
                        continue;
                    }
                    assert!(
                        b.origin.x >= px(0.) && b.origin.x + b.size.width <= viewport.width,
                        "{mode:?} @{width}px horizontal overflow on {:?}",
                        snap.path()
                    );
                }
            })
            .expect("window");
        }
    }
}

/// Tab moves cleanly across surrounding buttons without landing on `StatusBar` or any hint.
#[gpui_kit::test]
fn status_bar_and_hints_add_no_focus_stops(cx: &mut TestAppContext) {
    let window = open_bar(cx, 500., Mode::Dark, ("Ready", shell_hints()));
    cx.update_window(window, |_, window, cx| {
        window.press("tab", cx);
        assert_eq!(window.find("before-status").focused(), Some(true));

        window.press("tab", cx);
        assert_eq!(
            window.find("after-status").focused(),
            Some(true),
            "Tab from before-status must skip StatusBar and land directly on after-status"
        );
        for id in ["status-bar", "status-summary", "hint-new", "hint-close"] {
            assert_ne!(
                window.find(id).focused(),
                Some(true),
                "{id} must not be focusable"
            );
        }
    })
    .expect("window");
}
