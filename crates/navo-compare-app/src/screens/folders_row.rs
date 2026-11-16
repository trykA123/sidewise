//! Row drawing of the Folders view: tree lines, the folder mix bar and the summary chips.
//! Split from `folders_view` to keep both files short; the view owns the state.

use gpui_kit::base::TestSupportExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnimationExt, AnyElement, App, ElementId, Entity, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, div, img, px,
};
use navo_ui::browser::CompareFilter;

use super::folders_model::{Mix, TreeLine};
use super::folders_view::FoldersView;
use crate::icons::{cache::IconCache, icon_id};
use crate::motion;
use crate::theme::SidewiseTheme;
use crate::theme_data::change_syntax::ChangeKind;
use crate::theme_data::palettes::{DIMS, ModeTokens};

const INDENT: f32 = 16.; // px per depth level
const MARK: u32 = 16; // mark side, px
const MIX_W: f32 = 56.; // folder mix bar width, px

/// Everything one line needs to draw: the theme, the cursor, the fade and the owning view.
pub(super) struct LineCtx {
    pub theme: SidewiseTheme,
    pub m: ModeTokens,
    pub cursor: usize,
    pub fade: Option<(String, u64)>,
    pub filter: CompareFilter,
    pub view: Entity<FoldersView>,
}

/// The `ChangeKind` of each tone, in `SwTone` order (also the mix-bar order).
const KINDS: [ChangeKind; 5] = [
    ChangeKind::Chg,
    ChangeKind::Ol,
    ChangeKind::Or,
    ChangeKind::Cfl,
    ChangeKind::Same,
];

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
pub(super) fn line_view(line: &TreeLine, index: usize, ctx: &LineCtx, cx: &mut App) -> AnyElement {
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
pub(super) fn summary_bar(totals: Mix, ctx: &LineCtx) -> impl IntoElement + use<> {
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
