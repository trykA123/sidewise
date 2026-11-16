//! The header row above the Folders list: one block per side (L/R badge, folder name, path)
//! and `Result` over the gutter. Part of the side-by-side layout (NC133); `folders_view` owns it.

use std::path::{Path, PathBuf};

use gpui_kit::base::TestSupportExt as _;
use gpui_kit::{
    Div, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};

use super::folders_cells_view::gut_width;
use super::folders_row::LineCtx;
use crate::theme_data::palettes::DIMS;

/// The folder's own name, or the whole path when it has none (a drive root).
fn folder_name(root: &Path) -> String {
    let name = root.file_name().map(|name| name.to_string_lossy());
    name.map_or_else(|| root.display().to_string(), |name| name.into_owned())
}

/// One side's header: badge, bold folder name and a truncated mono path; its label is the path.
fn side_head(root: &Path, index: usize, badge: &'static str, ctx: &LineCtx) -> impl IntoElement {
    let path = SharedString::from(root.display().to_string());
    let m = ctx.m;
    let badge = div()
        .flex_none()
        .size(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(9.))
        .bg(m.accent_tint)
        .text_color(m.accent)
        .text_size(px(12.))
        .font_weight(FontWeight::BOLD)
        .child(badge);
    let who = div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .flex_1()
        .min_w_0()
        .child(
            div()
                .truncate()
                .text_size(px(13.5))
                .font_weight(FontWeight::SEMIBOLD)
                .child(SharedString::from(folder_name(root))),
        )
        .child(
            div()
                .truncate()
                .text_size(px(11.5))
                .font_family(ctx.mono.clone())
                .text_color(m.muted)
                .child(path.clone()),
        );
    div()
        .id(("head", index))
        .test_support()
        .aria_label(path)
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(9.))
        .px(DIMS.inset_x)
        .py(DIMS.inset_y)
        .child(badge)
        .child(who)
}

/// The header row: left side, `Result` at the gutter's width, right side; empty without roots.
pub(super) fn head_view(roots: Option<&[PathBuf; 2]>, ctx: &LineCtx) -> Option<Div> {
    let [left, right] = roots?;
    let centre = div()
        .flex_none()
        .w(px(gut_width(ctx.meta)))
        .flex()
        .items_center()
        .justify_center()
        .border_x_1()
        .border_color(ctx.m.sep)
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ctx.m.muted)
        .child("Result");
    Some(
        div()
            .flex()
            .items_center()
            .bg(ctx.m.card)
            .border_b_1()
            .border_color(ctx.m.sep)
            .child(side_head(left, 0, "L", ctx))
            .child(centre)
            .child(side_head(right, 1, "R", ctx)),
    )
}
