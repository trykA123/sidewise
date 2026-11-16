//! Tests of the tab menu: Close other tabs keeps the active tab, and the menu shows Ctrl+W.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext, point, px};

use crate::chrome::sidebar::SidebarEvent;
use crate::menu_host::menu_view;
use crate::shell::{Shell, bind_keys};
use crate::test_window::open;

/// Opens a shell with `tabs` tabs (keys bound) and renders one frame.
fn shell_with(cx: &mut TestAppContext, tabs: usize) -> (AnyWindowHandle, Entity<Shell>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        bind_keys(cx);
    });
    let (window, shell) = open(cx, Shell::new);
    cx.update_window(window, |_, window, cx| {
        shell.update(cx, |shell, cx| {
            for _ in 1..tabs {
                shell.new_tab(window, cx);
            }
        });
        window.render_frame(cx);
    })
    .expect("window");
    (window, shell)
}

/// Makes the sidebar ask for the tab menu of tab `index`, then settles.
fn ask_menu(
    cx: &mut TestAppContext,
    (window, shell): &(AnyWindowHandle, Entity<Shell>),
    index: usize,
) {
    cx.update_window(*window, |_, window, cx| {
        let sidebar = shell.read(cx).sidebar().clone();
        sidebar.update(cx, |_, cx| {
            cx.emit(SidebarEvent::SelectTab(index));
            cx.emit(SidebarEvent::TabMenu(index, point(px(30.), px(30.))));
        });
        window.render_frame(cx);
    })
    .expect("window");
    cx.run_until_parked();
}

/// Choosing Close other tabs leaves one tab, the one that was active.
#[gpui_kit::test]
fn close_others_keeps_the_active_tab(cx: &mut TestAppContext) {
    let opened = shell_with(cx, 4);
    let (window, shell) = &opened;
    let keep = cx
        .update_window(*window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.select_tab(2, window, cx));
            shell.read(cx).tab_ids()[2]
        })
        .expect("window");
    cx.update_window(*window, |_, window, cx| {
        shell.update(cx, |shell, cx| shell.close_others(window, cx));
    })
    .expect("window");
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.tab_ids(), vec![keep]);
        assert_eq!(shell.active(), 0);
    });
}

/// The menu lists Close tab with Ctrl+W, and dims Close other tabs when one tab is open.
#[gpui_kit::test]
fn the_tab_menu_shows_ctrl_w(cx: &mut TestAppContext) {
    let opened = shell_with(cx, 1);
    ask_menu(cx, &opened, 0);
    let menu = opened
        .1
        .read_with(cx, |shell, _| menu_view(&shell.menu))
        .expect("open menu");
    menu.read_with(cx, |menu, _| {
        let items = &menu.menu().items;
        assert_eq!(items[0].text, "Close tab");
        assert_eq!(items[0].accelerator.as_deref(), Some("Ctrl+W"));
        assert!(!items[1].enabled);
    });
}
