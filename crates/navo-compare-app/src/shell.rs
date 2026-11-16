//! The window's shell: the title bar, sidebar, topbar, active stage, toast host, and status bar.
//! It owns the tab list, syncs the sidebar, and handles browser-style tab keys (NS11, NS15, NS16),
//! the sidebar toggle, F6 pane cycling and hover-tip Escape forwarding.

use gpui_kit::base::TestSupportExt as _;
use gpui_kit::{App, FocusHandle, KeyBinding, Subscription, WindowAppearance};
use navo_compare_profile::appearance::PaletteId;
use navo_compare_profile::brand;
use navo_compare_profile::recent::StartRows;
use navo_ui::NavoTitleBar;
use navo_ui::prelude::*;
use navo_ui::tokens;

use crate::chrome::sidebar::{self, DismissTip, Sidebar, SidebarEvent, ToggleRail};
use crate::chrome::status_bar::{StatusBar, StatusHint};
use crate::chrome::topbar_status::{self, Action as TopbarAction, Crumbs, Topbar, TopbarMode};
use crate::layers::toast::ToastHost;
use crate::start::{self, StartScreen};
use crate::theme::{SidewiseTheme, apply_sidewise, mode_of};
use crate::theme_data::palettes::DIMS;

gpui_kit::actions!(shell, [NewTab, CloseTab, NextTab, PrevTab, CyclePane]);

/// Key context of the shell's root element; the tab keys are bound inside it.
const CONTEXT: &str = "Shell";

/// What a tab shows. Only the start screen exists so far; each start tab owns its screen.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TabKind {
    Start(Entity<StartScreen>),
}

/// One tab: the title in the strip and the kind of body it shows.
pub struct ShellTab {
    pub title: SharedString,
    pub kind: TabKind,
}

/// The shell view: the tab list, active index, sidebar, topbar, toast host, and focus handle.
pub struct Shell {
    tabs: Vec<ShellTab>,
    active: usize,
    sidebar: Entity<Sidebar>,
    _sidebar: Subscription,
    topbar: Entity<Topbar>,
    toasts: Entity<ToastHost>,
    handle: FocusHandle,
    _appearance: Subscription,
}

/// Binds the shell, start screen, sidebar, and topbar keys; call once after `navo_ui::init`.
pub fn bind_keys(cx: &mut App) {
    start::bind_keys(cx);
    sidebar::bind_keys(cx);
    topbar_status::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("ctrl-t", NewTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-w", CloseTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-tab", NextTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-shift-tab", PrevTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-\\", ToggleRail, Some(CONTEXT)),
        KeyBinding::new("f6", CyclePane, Some(CONTEXT)),
        KeyBinding::new("shift-f6", CyclePane, Some(CONTEXT)),
        KeyBinding::new("escape", DismissTip, Some(CONTEXT)),
    ]);
}

/// Extracts open tab titles in display order for initializing and syncing [`Sidebar`].
fn tab_titles(tabs: &[ShellTab]) -> Vec<SharedString> {
    tabs.iter().map(|tab| tab.title.clone()).collect()
}

/// A fresh start tab titled "New compare", with its own start screen over no stored rows yet.
fn start_tab(window: &mut Window, cx: &mut Context<Shell>) -> ShellTab {
    let cwd = std::env::current_dir().unwrap_or_default();
    let screen = cx.new(|cx| StartScreen::new(StartRows::default(), cwd, window, cx));
    ShellTab {
        title: "New compare".into(),
        kind: TabKind::Start(screen),
    }
}

impl Shell {
    /// Applies the active Sidewise palette (defaulting to Violet) in the appearance's mode.
    pub(crate) fn apply_appearance(appearance: WindowAppearance, cx: &mut App) {
        let palette = cx
            .try_global::<SidewiseTheme>()
            .map(|theme| theme.palette)
            .unwrap_or(PaletteId::Violet);
        apply_sidewise(palette, mode_of(appearance), cx);
    }

    /// Makes a shell with one start tab and focuses that tab's left field.
    /// It applies the active Sidewise palette in the window's mode and follows later changes (NS19).
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::apply_appearance(window.appearance(), cx);
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            Self::apply_appearance(window.appearance(), cx);
            cx.notify();
        });
        let tabs = vec![start_tab(window, cx)];
        let titles = tab_titles(&tabs);
        let sidebar = cx.new(|cx| Sidebar::new(StartRows::default(), titles, cx));
        let sidebar_sub = cx.subscribe_in(&sidebar, window, |this, _, event, window, cx| {
            this.on_sidebar(*event, window, cx);
        });
        let topbar = start_topbar(window, cx);
        let toasts = cx.new(|_| ToastHost::new());
        let shell = Self {
            tabs,
            active: 0,
            sidebar,
            _sidebar: sidebar_sub,
            topbar,
            toasts,
            handle: cx.focus_handle(),
            _appearance: appearance,
        };
        shell.activated(window, cx);
        shell
    }

    /// Number of open tabs.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "read by the view tests; the start screen reads it later"
        )
    )]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Index of the active tab.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "read by the view tests; the start screen reads it later"
        )
    )]
    pub fn active(&self) -> usize {
        self.active
    }

    /// The start screen entity ids of all tabs, in order (lets tests tell tabs apart).
    #[cfg(test)]
    pub(crate) fn tab_ids(&self) -> Vec<gpui_kit::EntityId> {
        self.tabs
            .iter()
            .map(|tab| match &tab.kind {
                TabKind::Start(screen) => screen.entity_id(),
            })
            .collect()
    }

    /// The sidebar entity (lets key tests read its rail state).
    #[cfg(test)]
    pub(crate) fn sidebar(&self) -> &Entity<Sidebar> {
        &self.sidebar
    }

    /// The start screen of the active tab.
    pub fn active_screen(&self) -> Option<&Entity<StartScreen>> {
        match self.tabs.get(self.active).map(|tab| &tab.kind) {
            Some(TabKind::Start(screen)) => Some(screen),
            None => None,
        }
    }

    /// Appends a start tab, makes it active and focuses its left field.
    pub fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = start_tab(window, cx);
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.sync_chrome(cx);
        self.activated(window, cx);
    }

    /// Closes the tab at `index`; the tab to its right (else the new last) becomes active.
    /// A tab left of the active one keeps the active tab. Closing the last tab closes the window.
    /// Only closing the active tab moves the focus; closing another tab leaves it. An index out
    /// of range does nothing.
    pub fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs.len() == 1 {
            window.remove_window();
            return;
        }
        let was_active = index == self.active;
        self.tabs.remove(index);
        if index < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.tabs.len() - 1);
        self.sync_chrome(cx);
        if was_active {
            self.activated(window, cx);
        } else {
            cx.notify();
        }
    }

    /// Makes the tab at `index` active; an index out of range does nothing.
    pub fn select_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            self.active = index;
            self.sync_chrome(cx);
            self.activated(window, cx);
        }
    }

    /// Moves to the next tab, wrapping from the last to the first.
    pub fn next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.select_tab((self.active + 1) % self.tabs.len(), window, cx);
    }

    /// Moves to the previous tab, wrapping from the first to the last.
    pub fn prev_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.tabs.len();
        self.select_tab((self.active + count - 1) % count, window, cx);
    }

    /// Focuses the active start tab's left field (NS16), else the shell; redraws.
    fn activated(&self, window: &mut Window, cx: &mut Context<Self>) {
        match self.active_screen() {
            Some(screen) => screen.update(cx, |screen, cx| screen.focus_left(window, cx)),
            None => self.handle.focus(window, cx),
        }
        cx.notify();
    }

    /// F6 / Shift+F6: moves focus to the other of the two panes, the sidebar or the active start
    /// screen. Does not resync tab data, so an overlay-only hover tip stays.
    fn cycle_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar.read(cx).contains_focus(window, cx) {
            self.activated(window, cx);
        } else {
            self.sidebar
                .update(cx, |sidebar, cx| sidebar.focus_primary(window, cx));
            cx.notify();
        }
    }

    /// `Escape` from Main: dismisses a shown hover tip through the sidebar, else passes on.
    fn dismiss_tip(&mut self, cx: &mut Context<Self>) {
        if !self
            .sidebar
            .update(cx, |sidebar, cx| sidebar.dismiss_tip(cx))
        {
            cx.propagate();
        }
    }

    /// Pushes open tab titles and active index into the sidebar without touching focus.
    fn sync_chrome(&self, cx: &mut Context<Self>) {
        let titles = tab_titles(&self.tabs);
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.set_tabs(titles, self.active, cx));
    }

    /// Routes typed sidebar actions to tab mutations or redraws on rail transitions.
    fn on_sidebar(&mut self, event: SidebarEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            SidebarEvent::NewCompare => self.new_tab(window, cx),
            SidebarEvent::SelectTab(index) => self.select_tab(index, window, cx),
            SidebarEvent::CloseTab(index) => self.close_tab(index, window, cx),
            SidebarEvent::RailToggled(_) => cx.notify(),
        }
    }

    /// Right-hand workspace column stacking the topbar, active stage, and status bar.
    fn main_pane(&self, cx: &App) -> impl IntoElement {
        div()
            .id("main-pane")
            .test_support()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(self.topbar.clone())
            .child(self.body(cx))
            .child(self.status_bar())
    }

    /// The active stage and bottom-right toast overlay slot; empty toasts do not cover inputs.
    fn body(&self, cx: &App) -> impl IntoElement {
        let background = tokens::current(cx).colors.background;
        let toasts = div()
            .id("toast-slot")
            .test_support()
            .absolute()
            .bottom(DIMS.inset_y)
            .right(DIMS.inset_x)
            .child(self.toasts.clone());
        div()
            .relative()
            .flex_1()
            .w_full()
            .min_h_0()
            .bg(background)
            .children(self.active_screen().cloned())
            .child(toasts)
    }

    /// Derives the status bar from active tab title, open-tab count, and working shortcuts.
    fn status_bar(&self) -> StatusBar {
        let count = self.tabs.len();
        let noun = if count == 1 { "open tab" } else { "open tabs" };
        let title = self
            .tabs
            .get(self.active)
            .map_or("", |tab| tab.title.as_ref());
        let summary = format!("{title} · {count} {noun}");
        let hints = [
            StatusHint {
                id: "hint-new".into(),
                keys: "ctrl-t".into(),
                label: "New tab".into(),
            },
            StatusHint {
                id: "hint-close".into(),
                keys: "ctrl-w".into(),
                label: "Close tab".into(),
            },
            StatusHint {
                id: "hint-sidebar".into(),
                keys: "ctrl-\\".into(),
                label: "Sidebar".into(),
            },
        ];
        StatusBar::new(summary, hints)
    }
}

/// Builds the start-mode topbar with all shown actions disabled until wired in later cuts.
fn start_topbar(window: &mut Window, cx: &mut Context<Shell>) -> Entity<Topbar> {
    let crumbs = Crumbs {
        title: "New compare".into(),
        detail: "Folder · File · Git".into(),
    };
    cx.new(|cx| {
        let mut bar = Topbar::new(crumbs, |_, _| {}, window, cx);
        bar.set_mode(TopbarMode::Start, cx);
        for action in [
            TopbarAction::Theme,
            TopbarAction::Accounts,
            TopbarAction::View,
            TopbarAction::Keys,
            TopbarAction::Sync,
        ] {
            bar.set_available(action, false, cx);
        }
        bar
    })
}

impl Render for Shell {
    /// Draws the full-width title bar above the sidebar and main workspace column.
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens::current(cx);
        let (background, text) = (t.colors.background, t.colors.text);
        let (font, size) = (t.typography.ui_font.clone(), t.typography.body);
        div()
            .key_context(CONTEXT)
            .track_focus(&self.handle)
            .on_action(cx.listener(|this, _: &NewTab, window, cx| this.new_tab(window, cx)))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                this.close_tab(this.active, window, cx);
            }))
            .on_action(cx.listener(|this, _: &NextTab, window, cx| this.next_tab(window, cx)))
            .on_action(cx.listener(|this, _: &PrevTab, window, cx| this.prev_tab(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleRail, _, cx| {
                this.sidebar
                    .update(cx, |sidebar, cx| sidebar.toggle_rail(cx));
            }))
            .on_action(cx.listener(|this, _: &CyclePane, window, cx| this.cycle_pane(window, cx)))
            .on_action(cx.listener(|this, _: &DismissTip, _, cx| this.dismiss_tip(cx)))
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
            .text_color(text)
            .font_family(font)
            .text_size(size)
            .child(NavoTitleBar::new(brand::NAME).menu_arrow(false))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.sidebar.clone())
                    .child(self.main_pane(cx)),
            )
    }
}
