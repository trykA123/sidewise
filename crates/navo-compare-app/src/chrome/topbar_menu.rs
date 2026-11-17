//! The View menu's anchor in the topbar (B4): the shell hands its open menu here, and the
//! topbar floats it under the View pill as a deferred overlay that takes no layout space.

use gpui_kit::{Anchor, AnyView, anchored, deferred};
use sidewise_kit::prelude::*;

use super::{Action, Topbar};

/// The gap between the View pill's bottom edge and the menu card, in px.
const MENU_GAP: f32 = 4.;

impl Topbar {
    /// Sets the menu that drops from the View pill; `None` when it is closed.
    pub fn set_menu(&mut self, menu: Option<AnyView>, cx: &mut Context<Self>) {
        self.menu = menu;
        cx.notify();
    }

    /// Wraps the `pill` of `action` with the open menu when it is the View pill: the menu's top
    /// right corner sits `MENU_GAP` below the pill's bottom right corner. Other pills pass through.
    pub(super) fn with_menu(&self, action: Action, pill: AnyElement) -> AnyElement {
        let Some(menu) = self.menu.clone().filter(|_| action == Action::View) else {
            return pill;
        };
        let overlay = anchored()
            .anchor(Anchor::TopRight)
            .offset(point(px(0.), px(MENU_GAP)))
            .child(menu);
        div()
            .relative()
            .flex_none()
            .child(pill)
            .child(
                div()
                    .absolute()
                    .top_full()
                    .right_0()
                    .child(deferred(overlay).with_priority(1)),
            )
            .into_any_element()
    }
}
