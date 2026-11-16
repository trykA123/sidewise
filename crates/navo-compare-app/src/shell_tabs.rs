//! Tab lifecycle of the shell: the tab types, and opening, closing, switching and tracking tabs.
//! A child module of `shell`, so it reads the shell's private fields.

use navo_compare_profile::recent::StartRows;
use navo_ui::prelude::*;

use super::Shell;
use crate::start::StartScreen;

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

/// Extracts open tab titles in display order for initializing and syncing [`Sidebar`].
pub(super) fn tab_titles(tabs: &[ShellTab]) -> Vec<SharedString> {
    tabs.iter().map(|tab| tab.title.clone()).collect()
}

/// A fresh start tab titled "New compare", with its own start screen over no stored rows yet.
pub(super) fn start_tab(window: &mut Window, cx: &mut Context<Shell>) -> ShellTab {
    let cwd = std::env::current_dir().unwrap_or_default();
    let screen = cx.new(|cx| StartScreen::new(StartRows::default(), cwd, window, cx));
    ShellTab {
        title: "New compare".into(),
        kind: TabKind::Start(screen),
    }
}
impl Shell {
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
    pub(super) fn activated(&self, window: &mut Window, cx: &mut Context<Self>) {
        match self.active_screen() {
            Some(screen) => screen.update(cx, |screen, cx| screen.focus_left(window, cx)),
            None => self.handle.focus(window, cx),
        }
        cx.notify();
    }

    /// Pushes open tab titles and active index into the sidebar without touching focus.
    pub(super) fn sync_chrome(&self, cx: &mut Context<Self>) {
        let titles = tab_titles(&self.tabs);
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.set_tabs(titles, self.active, cx));
    }
}
