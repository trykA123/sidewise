//! The shell's overlay layers: the Keys sheet and the View menu, and how the topbar reaches them.
//! A child module of `shell`, so it reads the shell's private fields. The drawer host stays unmounted.

use gpui_kit::{App, FocusHandle, Subscription, WeakEntity};
use navo_compare_profile::appearance::UiPrefs;
use navo_ui::prelude::*;

use super::Shell;
use crate::chrome::topbar_status::{Action, TopbarMode};
use crate::layers::keys_sheet::KeysSheet;
use crate::layers::view_menu::{ViewMenu, ViewMenuEvent};
use crate::theme_data::palettes::DIMS;

/// The open View menu, where focus was before it opened, and its event subscription.
struct OpenMenu {
    menu: Entity<ViewMenu>,
    restore: Option<FocusHandle>,
    _events: Subscription,
}

/// The layers the shell owns: the one Keys sheet, the View menu while open, and the edited View prefs.
pub(super) struct Layers {
    pub(super) keys: Entity<KeysSheet>,
    open: Option<OpenMenu>,
    /// The View menu's choices this session; no screen reads them yet, so they change nothing.
    prefs: UiPrefs,
}

impl Layers {
    /// The layers of a new shell: a closed Keys sheet and no View menu.
    pub(super) fn new(window: &mut Window, cx: &mut Context<Shell>) -> Self {
        Self {
            keys: cx.new(|cx| KeysSheet::new(window, cx)),
            open: None,
            prefs: UiPrefs::default(),
        }
    }
}

/// Whether the Sidewise View menu may open in `mode`: Start, Folders and Large. In File and Stream
/// `Alt+V` belongs to Navo's screen.
pub(super) fn view_available(mode: TopbarMode) -> bool {
    matches!(
        mode,
        TopbarMode::Start | TopbarMode::Folders | TopbarMode::Large
    )
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
    /// Routes a topbar action: Keys toggles the sheet, View toggles the menu; the rest do nothing.
    fn on_topbar(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::Keys => self
                .layers
                .keys
                .update(cx, |keys, cx| keys.toggle(window, cx)),
            Action::View => self.toggle_view_menu(window, cx),
            _ => {}
        }
    }

    /// Opens the View menu, or closes it when open. Does nothing where the menu is unavailable.
    fn toggle_view_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.layers.open.is_some() {
            return self.close_view_menu(window, cx);
        }
        if !self.topbar.read(cx).is_available(Action::View) {
            return;
        }
        let menu = cx.new(|cx| ViewMenu::new(self.layers.prefs.clone(), cx));
        let events = cx.subscribe_in(&menu, window, |this, menu, event, window, cx| {
            this.on_view_menu((menu, *event), window, cx);
        });
        let restore = window.focused(cx);
        menu.update(cx, |menu, cx| menu.focus(window, cx));
        self.layers.open = Some(OpenMenu {
            menu,
            restore,
            _events: events,
        });
        cx.notify();
    }

    /// Keeps the menu's choices on `Changed`; closes the menu on `Dismissed`.
    fn on_view_menu(
        &mut self,
        (menu, event): (&Entity<ViewMenu>, ViewMenuEvent),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ViewMenuEvent::Changed => self.layers.prefs = menu.read(cx).prefs().clone(),
            ViewMenuEvent::Dismissed => self.close_view_menu(window, cx),
        }
    }

    /// Closes the View menu and gives focus back to where it was.
    fn close_view_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.layers.open.take()
            && let Some(handle) = open.restore
        {
            window.focus(&handle, cx);
        }
        cx.notify();
    }

    /// `Alt+V` from the shell: opens the menu where it is available, else passes the key on.
    pub(super) fn view_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.layers.open.is_some() || self.topbar.read(cx).is_available(Action::View) {
            self.toggle_view_menu(window, cx);
        } else {
            cx.propagate();
        }
    }

    /// The View menu under the topbar's right edge while open; a click outside closes it.
    pub(super) fn view_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open = self.layers.open.as_ref()?;
        let card = div()
            .id("view-menu-slot")
            .absolute()
            .top(DIMS.inset_y)
            .right(DIMS.inset_x)
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_view_menu(window, cx)))
            .child(open.menu.clone());
        Some(card.into_any_element())
    }
}

impl Shell {
    /// Whether the Keys sheet shows; for tests.
    #[cfg(test)]
    pub(crate) fn keys_open(&self, cx: &App) -> bool {
        self.layers.keys.read(cx).is_open()
    }

    /// Whether the View menu shows; for tests.
    #[cfg(test)]
    pub(crate) fn view_menu_open(&self) -> bool {
        self.layers.open.is_some()
    }

    /// Whether the topbar's View action is available now; for tests.
    #[cfg(test)]
    pub(crate) fn view_action_available(&self, cx: &App) -> bool {
        self.topbar.read(cx).is_available(Action::View)
    }
}
