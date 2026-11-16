//! Tests of the drawer host: Ctrl+B and Ctrl+I widths, drawing with and without motion, and
//! the body slot.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, FocusHandle, TestAppContext};
use navo_compare_profile::appearance::PaletteId;
use navo_ui::tokens::Mode;

use super::*;
use crate::test_window::open;
use crate::theme::apply_sidewise;

/// A stand-in shell: the `Shell` context over a focusable `CompareView` body, and the host.
struct Root {
    drawers: Entity<DrawerHost>,
    body: FocusHandle,
}

impl Render for Root {
    /// Wires the actions the way the shell will and mounts the host beside the body.
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let body = div()
            .id("body")
            .key_context("CompareView")
            .track_focus(&self.body)
            .flex_1();
        let root = div().key_context("Shell").size_full().flex().child(body);
        wire(root, &self.drawers).child(self.drawers.clone())
    }
}

/// A window holding `Root` with the body focused and the keys bound.
fn root(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<DrawerHost>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        apply_sidewise(PaletteId::Violet, Mode::Light, cx);
        bind_keys(cx);
    });
    let (window, root) = open(cx, |window, cx| {
        let root = Root {
            drawers: cx.new(|_| DrawerHost::new(Drawer::Outline)),
            body: cx.focus_handle(),
        };
        window.focus(&root.body, cx);
        root
    });
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .expect("window");
    (window, root.read_with(cx, |root, _| root.drawers.clone()))
}

/// Ctrl+B and Ctrl+I move the drawer between 0 and 300 px, and a swap keeps it open.
#[gpui_kit::test]
fn ctrl_b_and_ctrl_i_switch_the_drawer(cx: &mut TestAppContext) {
    let (window, drawers) = root(cx);
    let state = |cx: &mut TestAppContext| {
        drawers.read_with(cx, |host, _| (host.open(), host.target_width()))
    };
    assert_eq!(state(cx), (None, 0.));
    cx.simulate_keystrokes(window, "ctrl-b");
    assert_eq!(state(cx), (Some(Drawer::Outline), 300.));
    cx.simulate_keystrokes(window, "ctrl-i");
    assert_eq!(state(cx), (Some(Drawer::Details), 300.), "swap, still open");
    cx.simulate_keystrokes(window, "ctrl-i");
    assert_eq!(state(cx), (None, 0.));
}

/// The drawer draws mid-animation and with reduced motion.
#[gpui_kit::test]
fn the_drawer_draws_animated_and_still(cx: &mut TestAppContext) {
    let (window, drawers) = root(cx);
    cx.simulate_keystrokes(window, "ctrl-b");
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        cx.set_reduce_motion(true);
        window.render_frame(cx);
    })
    .expect("window");
    assert_eq!(drawers.read_with(cx, |host, _| host.target_width()), 300.);
}

/// A plain view standing in for a screen's list.
struct Note;

impl Render for Note {
    /// One line of text.
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child("note")
    }
}

/// A body set twice for one drawer leaves one body, and Ctrl+B follows the screen's list kind.
#[gpui_kit::test]
fn a_drawer_keeps_one_body_and_the_list_kind_follows_the_screen(cx: &mut TestAppContext) {
    let host = cx.new(|_| DrawerHost::new(Drawer::Changes));
    let (first, second) = (cx.new(|_| Note), cx.new(|_| Note));
    cx.update(|cx| {
        host.update(cx, |host, cx| {
            host.set_body(Drawer::Changes, first.into(), cx);
            host.set_body(Drawer::Changes, second.into(), cx);
            host.toggle_list(cx);
        });
    });
    host.read_with(cx, |host, _| {
        assert_eq!(host.bodies.len(), 1, "a second body replaces the first");
        assert_eq!(
            host.open(),
            Some(Drawer::Changes),
            "Ctrl+B opens this screen's list"
        );
    });
}
