//! The Folders view of the compare tab: a summary bar and a tree of folders and files over the
//! `FolderTree` model. The cursor moves with the arrow keys; Enter opens a pair of files.

use std::rc::Rc;

use gpui_kit::base::TestSupportExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnimationExt, AnyElement, App, Context, ElementId, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, ReadGlobal, Render, ScrollStrategy,
    SharedString, StatefulInteractiveElement, Styled, UniformListScrollHandle, Window, div, img,
    px, uniform_list,
};
use navo_ui::browser::{CompareFilter, CompareRow};

use super::folders_model::{FolderTree, Mix, TreeLine};
use crate::icons::{cache::IconCache, icon_id};
use crate::motion;
use crate::theme::SidewiseTheme;
use crate::theme_data::change_syntax::ChangeKind;
use crate::theme_data::palettes::{DIMS, ModeTokens};

gpui_kit::actions!(folders, [CursorUp, CursorDown, StepIn, StepOut, Activate]);

/// Key context of the view root; the cursor keys bind inside it.
const CONTEXT: &str = "Folders";
const INDENT: f32 = 16.; // px per depth level
const MARK: u32 = 16; // mark side, px
const MIX_W: f32 = 56.; // folder mix bar width, px

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
        Self {
            tree: FolderTree::new(Vec::new()),
            cursor: 0,
            cursor_path: None,
            fade: None,
            opened: 0,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
        }
    }

    /// Replaces the rows; the cursor stays on its path while that line still shows.
    pub fn set_rows(&mut self, rows: Vec<CompareRow>, cx: &mut Context<Self>) {
        self.tree.set_rows(rows);
        self.sync_cursor();
        cx.notify();
    }

    /// Opens the ancestors of `path` and puts the cursor on it; false when it is not shown.
    pub fn select_path(&mut self, path: &str, cx: &mut Context<Self>) -> bool {
        for (index, _) in path.match_indices('/') {
            self.tree.set_expanded(&path[..index], true);
        }
        let lines = self.tree.lines();
        let at = lines.iter().position(|line| line.path == path);
        at.map(|at| self.move_to(at, &lines, cx)).is_some()
    }

    /// The path of the line under the cursor.
    pub fn selected_path(&self) -> Option<String> {
        self.cursor_path.clone()
    }

    /// Puts the cursor back on its path, or on the nearest line when the path is gone.
    fn sync_cursor(&mut self) {
        let lines = self.tree.lines();
        let found = self
            .cursor_path
            .as_ref()
            .and_then(|path| lines.iter().position(|line| &line.path == path));
        self.cursor = found.unwrap_or(self.cursor.min(lines.len().saturating_sub(1)));
        self.cursor_path = lines.get(self.cursor).map(|line| line.path.clone());
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
        self.sync_cursor();
        cx.notify();
    }

    /// Sets the filter from a chip click.
    fn set_filter(&mut self, filter: CompareFilter, cx: &mut Context<Self>) {
        self.tree.set_filter(filter);
        self.sync_cursor();
        cx.notify();
    }

    /// A click on line `at`: the cursor goes there and a folder toggles.
    fn click_line(&mut self, at: usize, cx: &mut Context<Self>) {
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

/// Everything one line needs to draw: the theme, the cursor, the fade and the owning view.
struct LineCtx {
    theme: SidewiseTheme,
    m: ModeTokens,
    cursor: usize,
    fade: Option<(String, u64)>,
    filter: CompareFilter,
    view: gpui_kit::Entity<FoldersView>,
}

/// The `ChangeKind` of each tone, in `SwTone` order (also the mix-bar order).
#[rustfmt::skip]
const KINDS: [ChangeKind; 5] = [ChangeKind::Chg, ChangeKind::Ol, ChangeKind::Or, ChangeKind::Cfl, ChangeKind::Same];

/// A 56 px bar of five segments, each as wide as its share of `mix`.
fn mix_bar(mix: Mix, ctx: &LineCtx) -> impl IntoElement {
    let total: u32 = mix.iter().sum();
    let bar = div()
        .flex()
        .h(px(4.))
        .w(px(MIX_W))
        .rounded_full()
        .overflow_hidden();
    bar.children(
        KINDS
            .iter()
            .zip(mix)
            .filter(|(_, n)| *n > 0)
            .map(|(kind, n)| {
                let mark = ctx.theme.change.tone(*kind, ctx.theme.mode).mark;
                div()
                    .h_full()
                    .w(px(MIX_W * n as f32 / total as f32))
                    .bg(mark)
            }),
    )
}

/// The folder or file mark of `line` at the 1x bucket; empty when the raster is missing.
fn mark_of(line: &TreeLine, cx: &mut App) -> impl IntoElement {
    let image = IconCache::image(icon_id(&line.name, line.folder, line.expanded), MARK, cx);
    let side = px(MARK as f32);
    div()
        .size(side)
        .flex_none()
        .children(image.map(|image| img(image).size(side)))
}

/// One 30 px line: indent, mark, name, and the mix bar of a closed folder.
fn line_view(line: &TreeLine, index: usize, ctx: &LineCtx, cx: &mut App) -> AnyElement {
    let tone = line
        .tone
        .map(|tone| ctx.theme.change.tone(KINDS[tone as usize], ctx.theme.mode));
    let view = ctx.view.clone();
    let row = div()
        .id(("folder-line", index))
        .test_support()
        .flex()
        .items_center()
        .gap_2()
        .h(DIMS.row_h)
        .pl(px(8. + INDENT * f32::from(line.depth)))
        .text_size(DIMS.fs)
        .text_color(tone.map_or(ctx.m.text, |tone| tone.fg))
        .when(index == ctx.cursor, |row| row.bg(ctx.m.selected))
        .on_click(move |_, _, cx| view.update(cx, |this, cx| this.click_line(index, cx)))
        .child(mark_of(line, cx))
        .child(
            div()
                .flex_1()
                .truncate()
                .child(SharedString::from(line.name.clone())),
        );
    let row = match line.folder && !line.expanded && line.mix.iter().any(|n| *n > 0) {
        true => row.child(mix_bar(line.mix, ctx)),
        false => row,
    };
    fade_in(row, (line, index), ctx, cx)
}

/// Wraps `row` in the opacity fade when it sits in the subtree that just opened.
fn fade_in(
    row: impl IntoElement,
    (line, index): (&TreeLine, usize),
    ctx: &LineCtx,
    cx: &App,
) -> AnyElement {
    let inside = |(path, n): &(String, u64)| {
        let rest = line.path.strip_prefix(path.as_str())?;
        rest.starts_with('/').then_some(*n)
    };
    let Some(generation) = ctx.fade.as_ref().and_then(inside) else {
        return row.into_any_element();
    };
    let id = ElementId::from((SharedString::from(format!("fade-{generation}")), index));
    match motion::anim(id.clone(), motion::row(cx).as_millis() as u64, cx) {
        Some(anim) => div()
            .child(row)
            .with_animation(id, anim, |el, delta| el.opacity(delta))
            .into_any_element(),
        None => row.into_any_element(),
    }
}

/// One filter chip: a label with its file count; the active filter shows in the accent.
fn chip_view(label: &str, count: u32, filter: CompareFilter, ctx: &LineCtx) -> AnyElement {
    let view = ctx.view.clone();
    let (fg, bg) = match filter == ctx.filter {
        true => (ctx.m.accent, ctx.m.accent_tint),
        false => (ctx.m.muted, ctx.m.layer2),
    };
    div()
        .id(SharedString::from(format!("chip-{label}")))
        .test_support()
        .px_3()
        .h(px(24.))
        .flex()
        .items_center()
        .rounded_full()
        .text_size(DIMS.fs)
        .text_color(fg)
        .bg(bg)
        .on_click(move |_, _, cx| view.update(cx, |this, cx| this.set_filter(filter, cx)))
        .child(SharedString::from(format!("{label} {count}")))
        .into_any_element()
}

/// The summary bar: chips for All, Differences, Only left/right and Same, counted from `totals`.
fn summary_bar(totals: Mix, ctx: &LineCtx) -> impl IntoElement + use<> {
    let [chg, ol, or, cfl, same] = totals;
    let chips = [
        ("All", totals.iter().sum(), CompareFilter::All),
        (
            "Differences",
            chg + ol + or + cfl,
            CompareFilter::Differences,
        ),
        ("Only left/right", ol + or, CompareFilter::Orphans),
        ("Same", same, CompareFilter::Same),
    ];
    let chips = chips.map(|(label, count, filter)| chip_view(label, count, filter, ctx));
    div().flex().gap_2().p_2().children(chips)
}

impl Render for FoldersView {
    /// The summary chips above a virtual list of tree lines.
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
        let list = uniform_list("folder-lines", lines.len(), move |range, _, cx| {
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
            .flex_col()
            .size_full()
            .child(bar)
            .child(list)
    }
}

#[cfg(test)]
#[path = "folders_view_tests.rs"]
mod tests;
