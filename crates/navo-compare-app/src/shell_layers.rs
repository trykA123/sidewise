//! The shell's overlay layers: the Keys sheet and the View menu, and how the topbar reaches them.
//! A child module of `shell`. The drawer host stays unmounted (NC119).

use gpui_kit::{App, Subscription, WeakEntity};
use sidewise_kit::prelude::*;

use super::Shell;
use crate::chrome::topbar_status::Action;
use crate::layers::keys_sheet::KeysSheet;
use crate::layers::view_menu::{ViewMenu, ViewMenuEvent};
use crate::screens::compare_host::look_of;
use crate::shell_prefs;

/// The layers the shell owns: the one Keys sheet and the View menu while it is open.
pub(super) struct Layers {
    pub(super) keys: Entity<KeysSheet>,
    pub(super) view: Option<(Entity<ViewMenu>, Subscription)>,
}

impl Layers {
    /// The layers of a new shell: a closed Keys sheet.
    pub(super) fn new(window: &mut Window, cx: &mut Context<Shell>) -> Self {
        Self {
            keys: cx.new(|cx| KeysSheet::new(window, cx)),
            view: None,
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
    /// Routes a topbar action: Keys toggles the sheet, View toggles the menu; Sync does nothing.
    fn on_topbar(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::Keys => {
                self.layers
                    .keys
                    .update(cx, |keys, cx| keys.toggle(window, cx));
            }
            Action::View if self.layers.view.is_some() => {
                self.layers.view = None;
                self.activated(window, cx);
            }
            Action::View => self.open_view_menu(window, cx),
            _ => {}
        }
    }

    /// Alt+V from anywhere in the shell: acts like the View pill while the topbar offers View.
    pub(super) fn on_view_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.topbar.read(cx).is_available(Action::View) {
            self.on_topbar(Action::View, window, cx);
        }
    }

    /// Opens the View menu over the saved preferences and focuses it.
    fn open_view_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prefs = shell_prefs::prefs(cx);
        let menu = cx.new(|cx| ViewMenu::new(prefs, cx));
        let sub = cx.subscribe_in(&menu, window, |this, menu, event, window, cx| match event {
            ViewMenuEvent::Changed => {
                let prefs = menu.read(cx).prefs().clone();
                if let Err(message) = shell_prefs::save_prefs(prefs, cx) {
                    this.toasts
                        .update(cx, |toasts, cx| toasts.push(message, cx));
                }
                this.apply_look(cx);
            }
            ViewMenuEvent::Dismissed => {
                this.layers.view = None;
                this.activated(window, cx);
            }
        });
        let handle = menu.clone();
        handle.update(cx, |menu, cx| menu.focus(window, cx));
        self.layers.view = Some((menu, sub));
        cx.notify();
    }

    /// Hands the saved look to every open compare tab.
    fn apply_look(&mut self, cx: &mut Context<Self>) {
        let look = look_of(&shell_prefs::prefs(cx));
        for tab in self.compare_tabs() {
            tab.update(cx, |tab, cx| tab.set_look(look, cx));
        }
    }

    /// The open View menu, for the render.
    pub(super) fn view_menu(&self) -> Option<Entity<ViewMenu>> {
        self.layers.view.as_ref().map(|(menu, _)| menu.clone())
    }

    /// Whether the Keys sheet shows; for tests.
    #[cfg(test)]
    pub(crate) fn keys_open(&self, cx: &App) -> bool {
        self.layers.keys.read(cx).is_open()
    }

    /// Whether the View menu shows; for tests.
    #[cfg(test)]
    pub(crate) fn view_open(&self) -> bool {
        self.layers.view.is_some()
    }

    /// Whether the topbar's View action is available now; for tests.
    #[cfg(test)]
    pub(crate) fn view_action_available(&self, cx: &App) -> bool {
        self.topbar.read(cx).is_available(Action::View)
    }
}
