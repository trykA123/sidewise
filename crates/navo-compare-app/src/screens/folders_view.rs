//! The Folders view of the compare tab: a summary bar and a tree of folders and files over the
//! `FolderTree` model. The cursor moves with the arrow keys; Enter opens a pair of files.

use std::rc::Rc;

use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, ReadGlobal, Render, ScrollStrategy,
    Styled, UniformListScrollHandle, Window, div, px, uniform_list,
};
use navo_ui::browser::{CompareFilter, CompareRow};
use navo_ui::compare_diff::map::{ChangeMap, MapEvent};

use super::folders_model::{FolderTree, TreeLine, map_hunks};
use super::folders_row::{LineCtx, line_view, summary_bar};
use crate::theme::SidewiseTheme;

gpui_kit::actions!(folders, [CursorUp, CursorDown, StepIn, StepOut, Activate]);

/// Key context of the view root; the cursor keys bind inside it.
const CONTEXT: &str = "Folders";

/// What the view tells its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoldersEvent {
    /// Enter on a file line: its row id.
    OpenPair(u64),
}

/// Binds the cursor keys inside the `Folders` context; the shell calls this once at start.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", CursorUp, Some(CONTEXT)),
        KeyBinding::new("down", CursorDown, Some(CONTEXT)),
        KeyBinding::new("right", StepIn, Some(CONTEXT)),
        KeyBinding::new("left", StepOut, Some(CONTEXT)),
        KeyBinding::new("enter", Activate, Some(CONTEXT)),
    ]);
}

/// The folder tree, the cursor and the scroll state of one Folders view.
pub struct FoldersView {
    tree: FolderTree,
    cursor: usize,
    cursor_path: Option<String>,
    fade: Option<(String, u64)>,
    opened: u64,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    map: Entity<ChangeMap>,
}

impl EventEmitter<FoldersEvent> for FoldersView {}

impl Focusable for FoldersView {
    /// The handle the view's key context tracks.
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl FoldersView {
    /// An empty view with the cursor on the first line once rows arrive.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let map = cx.new(|_| ChangeMap::new(map_hunks(&[]), 0).with_width(px(14.)));
        cx.subscribe(&map, |this, _, event: &MapEvent, cx| {
            let MapEvent::Jump(row) = *event;
            this.jump_to(row as usize, cx);
        })
        .detach();
        Self {
            tree: FolderTree::new(Vec::new()),
            cursor: 0,
            cursor_path: None,
            fade: None,
            opened: 0,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            map,
        }
    }

    /// Replaces the rows; the cursor stays on its path while that line still shows.
    pub fn set_rows(&mut self, rows: Vec<CompareRow>, cx: &mut Context<Self>) {
        self.tree.set_rows(rows);
        self.sync_cursor(cx);
        cx.notify();
    }

    /// Opens the ancestors of `path` and puts the cursor on it; false when it is not shown.
    pub fn select_path(&mut self, path: &str, cx: &mut Context<Self>) -> bool {
        for (index, _) in path.match_indices('/') {
            self.tree.set_expanded(&path[..index], true);
        }
        self.sync_cursor(cx);
        let lines = self.tree.lines();
        let at = lines.iter().position(|line| line.path == path);
        at.map(|at| self.move_to(at, &lines, cx)).is_some()
    }

    /// The path of the line under the cursor.
    pub fn selected_path(&self) -> Option<String> {
        self.cursor_path.clone()
    }

    /// Puts the cursor back on its path, or on the nearest line when the path is gone; the
    /// change map follows the new lines.
    fn sync_cursor(&mut self, cx: &mut Context<Self>) {
        let lines = self.tree.lines();
        let found = self
            .cursor_path
            .as_ref()
            .and_then(|path| lines.iter().position(|line| &line.path == path));
        self.cursor = found.unwrap_or(self.cursor.min(lines.len().saturating_sub(1)));
        self.cursor_path = lines.get(self.cursor).map(|line| line.path.clone());
        let (hunks, total) = (map_hunks(&lines), lines.len() as u32);
        self.map
            .update(cx, |map, cx| map.set_hunks(hunks, total, cx));
    }

    /// A jump from the change map: the cursor goes to line `row`, centred in the list.
    fn jump_to(&mut self, row: usize, cx: &mut Context<Self>) {
        let lines = self.tree.lines();
        if row < lines.len() {
            self.move_to(row, &lines, cx);
            self.scroll.scroll_to_item(row, ScrollStrategy::Center);
        }
    }

    /// Moves the cursor to line `at` of `lines` and scrolls it into view.
    fn move_to(&mut self, at: usize, lines: &[TreeLine], cx: &mut Context<Self>) {
        self.cursor = at;
        self.cursor_path = lines.get(at).map(|line| line.path.clone());
        self.scroll.scroll_to_item(at, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Opens or closes the folder `path`; an opened folder fades its subtree in.
    fn set_open(&mut self, path: &str, open: bool, cx: &mut Context<Self>) {
        if self.tree.set_expanded(path, open) && open {
            self.opened += 1;
            self.fade = Some((path.to_string(), self.opened));
        }
        self.sync_cursor(cx);
        cx.notify();
    }

    /// Sets the filter from a chip click.
    pub(super) fn set_filter(&mut self, filter: CompareFilter, cx: &mut Context<Self>) {
        self.tree.set_filter(filter);
        self.sync_cursor(cx);
        cx.notify();
    }

    /// A click on line `at`: the cursor goes there and a folder toggles.
    pub(super) fn click_line(&mut self, at: usize, cx: &mut Context<Self>) {
        let lines = self.tree.lines();
        let Some(line) = lines.get(at) else { return };
        self.move_to(at, &lines, cx);
        if line.folder {
            self.set_open(&line.path, !line.expanded, cx);
        }
    }

    /// Up (-1) or down (+1) one line, stopping at the ends.
    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let lines = self.tree.lines();
        let at = self.cursor.saturating_add_signed(delta);
        if at < lines.len() {
            self.move_to(at, &lines, cx);
        }
    }

    /// The lines and the line under the cursor; `None` while the tree is empty.
    fn current(&self) -> Option<(Vec<TreeLine>, TreeLine)> {
        let lines = self.tree.lines();
        let line = lines.get(self.cursor)?.clone();
        Some((lines, line))
    }

    /// Right: opens a closed folder, or moves to the first child of an open one.
    fn step_in(&mut self, cx: &mut Context<Self>) {
        let Some((lines, line)) = self.current().filter(|(_, line)| line.folder) else {
            return;
        };
        match (line.expanded, lines.get(self.cursor + 1)) {
            (false, _) => self.set_open(&line.path, true, cx),
            (true, Some(next)) if next.depth > line.depth => {
                self.move_to(self.cursor + 1, &lines, cx)
            }
            _ => {}
        }
    }

    /// Left: closes an open folder, or moves to the parent line.
    fn step_out(&mut self, cx: &mut Context<Self>) {
        let Some((lines, line)) = self.current() else {
            return;
        };
        if line.folder && line.expanded {
            return self.set_open(&line.path, false, cx);
        }
        let parent = line.path.rsplit_once('/').map(|(parent, _)| parent);
        let at = lines
            .iter()
            .position(|other| Some(other.path.as_str()) == parent);
        if let Some(at) = at {
            self.move_to(at, &lines, cx);
        }
    }

    /// Enter: toggles a folder, or asks the parent to open the file pair.
    fn activate(&mut self, cx: &mut Context<Self>) {
        let Some((_, line)) = self.current() else {
            return;
        };
        match line.folder {
            true => self.set_open(&line.path, !line.expanded, cx),
            false => cx.emit(FoldersEvent::OpenPair(line.id)),
        }
    }
}

impl Render for FoldersView {
    /// The summary chips above a virtual list of tree lines, with the change map at the right.
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *SidewiseTheme::global(cx);
        let m = if theme.mode.is_dark() {
            theme.spec.dark
        } else {
            theme.spec.light
        };
        let (cursor, fade, filter) = (self.cursor, self.fade.clone(), self.tree.filter());
        let ctx = LineCtx {
            theme,
            m,
            cursor,
            fade,
            filter,
            view: cx.entity(),
        };
        let lines = Rc::new(self.tree.lines());
        let bar = summary_bar(self.tree.totals(), &ctx);
        let map = self.map.clone();
        let list = uniform_list("folder-lines", lines.len(), move |range, _, cx| {
            let seen = range.start as u32..range.end as u32;
            map.update(cx, |map, cx| map.set_viewport(seen, cx));
            range
                .map(|index| line_view(&lines[index], index, &ctx, cx))
                .collect()
        })
        .track_scroll(&self.scroll)
        .flex_1();
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &CursorUp, _, cx| this.step(-1, cx)))
            .on_action(cx.listener(|this, _: &CursorDown, _, cx| this.step(1, cx)))
            .on_action(cx.listener(|this, _: &StepIn, _, cx| this.step_in(cx)))
            .on_action(cx.listener(|this, _: &StepOut, _, cx| this.step_out(cx)))
            .on_action(cx.listener(|this, _: &Activate, _, cx| this.activate(cx)))
            .flex()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(bar)
                    .child(list),
            )
            .child(self.map.clone())
    }
}

#[cfg(test)]
#[path = "folders_view_tests.rs"]
mod tests;
