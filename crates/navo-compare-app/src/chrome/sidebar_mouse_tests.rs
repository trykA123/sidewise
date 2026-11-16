//! Mouse tests of `Sidebar` rows: a middle click on an open tab closes it.

use gpui_kit::test::{ClickOptions, TestWindowExt};
use gpui_kit::{AppContext as _, MouseButton, TestAppContext};

use super::super::SidebarEvent;
use super::super::sidebar_tests::open_host_with_outside;

/// A middle click on tab B emits `CloseTab(1)` and nothing else, like its close button.
#[gpui_kit::test]
fn a_middle_click_closes_that_tab(cx: &mut TestAppContext) {
    let rig = open_host_with_outside(cx);
    cx.update_window(rig.window, |_, window, cx| {
        rig.view.update(cx, |sb, cx| {
            sb.set_tabs(vec!["A".into(), "B".into(), "C".into()], 0, cx)
        });
        window.render_frame(cx);
        window.click_with_options(
            ("open-tab", 1usize),
            ClickOptions::new().with_button(MouseButton::Middle),
            cx,
        );
        window.render_frame(cx);
    })
    .expect("middle click");
    assert_eq!(rig.events.borrow().last(), Some(&SidebarEvent::CloseTab(1)));
}
