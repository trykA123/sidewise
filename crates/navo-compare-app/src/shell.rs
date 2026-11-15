//! The window's shell: the title bar, tabs, topbar, active stage, toast host, and status bar.
//! It owns the tab list and the browser-style tab keys (NS11, NS15, NS16).

use gpui_kit::assets::IconName;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::{App, FocusHandle, KeyBinding, Subscription, WindowAppearance};
use navo_compare_profile::appearance::PaletteId;
use navo_compare_profile::brand;
use navo_compare_profile::recent::StartRows;
use navo_ui::NavoTitleBar;
use navo_ui::prelude::*;
use navo_ui::primitives::{IconButton, Tab, Tabs};
use navo_ui::tokens;

use crate::chrome::status_bar::{StatusBar, StatusHint};
use crate::chrome::topbar_status::{Action as TopbarAction, Crumbs, Topbar, TopbarMode};
use crate::layers::toast::ToastHost;
use crate::start::{self, StartScreen};
use crate::theme::{SidewiseTheme, apply_sidewise, mode_of};
use crate::theme_data::palettes::DIMS;

gpui_kit::actions!(shell, [NewTab, CloseTab, NextTab, PrevTab]);

/// Key context of the shell's root element; the tab keys are bound inside it.
const CONTEXT: &str = "Shell";

/// Width of the "+" button at the end of the tab row (NS15).
const PLUS_WIDTH: f32 = 28.;

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

/// The shell view: the tab list, active index, topbar, toast host, and focus handle.
pub struct Shell {
    tabs: Vec<ShellTab>,
    active: usize,
    topbar: Entity<Topbar>,
    toasts: Entity<ToastHost>,
    handle: FocusHandle,
    _appearance: Subscription,
}

/// Binds the tab keys in the `Shell` context and the start screen's keys;
/// call once after `navo_ui::init`.
pub fn bind_keys(cx: &mut App) {
    start::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("ctrl-t", NewTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-w", CloseTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-tab", NextTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-shift-tab", PrevTab, Some(CONTEXT)),
    ]);
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
        let topbar = start_topbar(window, cx);
        let toasts = cx.new(|_| ToastHost::new());
        let shell = Self {
            tabs: vec![start_tab(window, cx)],
            active: 0,
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

    /// The tab strip with the "+" button at its end.
    fn tab_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens::current(cx);
        let plus = IconButton::new("new-tab", IconName::Plus, "New tab")
            .on_click(cx.listener(|this, _, window, cx| this.new_tab(window, cx)));
        let tabs = Tabs::new(
            "tabs",
            self.tabs.iter().map(|tab| Tab::new(tab.title.clone())),
        )
        .selected(self.active)
        .trailing(plus, px(PLUS_WIDTH))
        .on_select(cx.listener(|this, index: &usize, window, cx| {
            this.select_tab(*index, window, cx);
        }))
        .on_close(cx.listener(|this, index: &usize, window, cx| {
            this.close_tab(*index, window, cx);
        }));
        div()
            .w_full()
            .flex_none()
            .px(t.spacing.sm)
            .py(t.spacing.xs)
            .border_b_1()
            .border_color(t.colors.line)
            .child(tabs)
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
    /// Draws the title bar, tab row, topbar, active stage with toast overlay, and status bar.
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
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
            .text_color(text)
            .font_family(font)
            .text_size(size)
            .child(NavoTitleBar::new(brand::NAME))
            .child(self.tab_row(cx))
            .child(self.topbar.clone())
            .child(self.body(cx))
            .child(self.status_bar())
    }
}
