//! Keyboard order of the start screen: the six stops Tab and Shift+Tab walk, and who holds focus.
//! It extends `StartScreen`; the tab cells' focus handles come from `StartTabs`.

use sidewise_kit::prelude::*;

use crate::start::{Side, StartScreen};

/// One keyboard stop of the start screen, in Tab (visual) order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stop {
    Folder,
    File,
    Left,
    Right,
    Compare,
    Rule,
}

/// Every stop, in Tab order; Tab wraps from the last to the first.
const STOPS: [Stop; 6] = [
    Stop::Folder,
    Stop::File,
    Stop::Left,
    Stop::Right,
    Stop::Compare,
    Stop::Rule,
];

impl StartScreen {
    /// The stop that holds the focus, if any.
    pub(crate) fn focused_stop(&self, window: &Window, cx: &App) -> Option<Stop> {
        let tabs = self.tabs.read(cx);
        STOPS.into_iter().find(|&stop| match stop {
            Stop::Left => self.focused_side(window, cx) == Some(Side::Left),
            Stop::Right => self.focused_side(window, cx) == Some(Side::Right),
            _ => tabs.handle(stop).is_some_and(|h| h.is_focused(window)),
        })
    }

    /// Tab and Shift+Tab: the next or previous stop, wrapping; from outside the stops (the
    /// list) the left field.
    pub(crate) fn cycle_stop(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        let next = self.focused_stop(window, cx).map_or(Stop::Left, |stop| {
            let at = STOPS.iter().position(|&s| s == stop).unwrap_or(0);
            STOPS[(at + if back { STOPS.len() - 1 } else { 1 }) % STOPS.len()]
        });
        match next {
            Stop::Left => self.focus_side(Side::Left, window, cx),
            Stop::Right => self.focus_side(Side::Right, window, cx),
            _ => {
                if let Some(handle) = self.tabs.read(cx).handle(next).cloned() {
                    handle.focus(window, cx);
                }
            }
        }
    }
}
