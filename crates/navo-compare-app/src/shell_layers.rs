//! The shell's overlay layers: the Keys sheet, and how the topbar reaches it.
//! A child module of `shell`. The View menu and the drawer host stay unmounted (NC123, NC119).

use gpui_kit::{App, WeakEntity};
use navo_ui::prelude::*;

use super::Shell;
use crate::chrome::topbar_status::Action;
use crate::layers::keys_sheet::KeysSheet;

/// The layers the shell owns: the one Keys sheet.
pub(super) struct Layers {
    pub(super) keys: Entity<KeysSheet>,
}

impl Layers {
    /// The layers of a new shell: a closed Keys sheet.
    pub(super) fn new(window: &mut Window, cx: &mut Context<Shell>) -> Self {
        Self {
            keys: cx.new(|cx| KeysSheet::new(window, cx)),
        }
    }
}

/// The callback for the topbar: it hands each action to the shell once the current update ends,
/// because the topbar fires it inside its own update and without a window.
pub(super) fn topbar_router(
    shell: WeakEntity<Shell>,
    window: &Window,
) -> impl Fn(Action, &mut App) + 'static {
    let handle = window.window_handle();
    move |action, cx| {
        let shell = shell.clone();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.on_topbar(action, window, cx));
            });
        });
    }
}

impl Shell {
    /// Routes a topbar action: Keys toggles the sheet; the rest do nothing in alpha.1.
    fn on_topbar(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if action == Action::Keys {
            self.layers
                .keys
                .update(cx, |keys, cx| keys.toggle(window, cx));
        }
    }

    /// Whether the Keys sheet shows; for tests.
    #[cfg(test)]
    pub(crate) fn keys_open(&self, cx: &App) -> bool {
        self.layers.keys.read(cx).is_open()
    }

    /// Whether the topbar's View action is available now; for tests.
    #[cfg(test)]
    pub(crate) fn view_action_available(&self, cx: &App) -> bool {
        self.topbar.read(cx).is_available(Action::View)
    }
}
