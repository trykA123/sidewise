//! Presentational status bar (NC66): muted summary left and non-focusable `Kbd` hints right.

use gpui_kit::{RenderOnce, Role, base::TestSupportExt as _, div};
use navo_ui::{prelude::*, primitives::Kbd, tokens};

/// One keyboard hint displayed on the right side of [`StatusBar`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusHint {
    pub id: ElementId,
    pub keys: SharedString,
    pub label: SharedString,
}

/// Presentational status bar showing a truncated summary left and valid key hints right.
#[derive(IntoElement)]
pub struct StatusBar {
    summary: SharedString,
    hints: Vec<StatusHint>,
}

impl StatusBar {
    /// Creates a presentational status bar from caller-owned summary text and key hints.
    pub fn new(
        summary: impl Into<SharedString>,
        hints: impl IntoIterator<Item = StatusHint>,
    ) -> Self {
        Self {
            summary: summary.into(),
            hints: hints.into_iter().collect(),
        }
    }
}

/// Builds one non-focusable `Kbd` + label hint row, omitting invalid key strings.
fn hint_view(hint: StatusHint, gap: Pixels) -> Option<Div> {
    let kbd = Kbd::new(hint.id, &hint.keys)?;
    Some(
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(gap)
            .child(kbd)
            .child(hint.label),
    )
}

impl RenderOnce for StatusBar {
    /// Draws the footer on matte surface (`rail`) with a hairline (`line`), keeping full summary text.
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens::current(cx);
        let summary = div()
            .id("status-summary")
            .role(Role::Label)
            .aria_label(self.summary.clone())
            .test_support()
            .flex_1()
            .min_w_0()
            .whitespace_nowrap()
            .overflow_hidden()
            .text_ellipsis()
            .child(self.summary);
        let hints = self
            .hints
            .into_iter()
            .filter_map(|h| hint_view(h, t.spacing.sm));
        let keys = div()
            .id("status-hints")
            .test_support()
            .flex()
            .min_w_0()
            .items_center()
            .gap(t.spacing.lg)
            .overflow_hidden()
            .children(hints);
        div()
            .id("status-bar")
            .role(Role::Status)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(t.spacing.lg)
            .w_full()
            .overflow_hidden()
            .h(t.spacing.status_height)
            .px(t.spacing.xxl)
            .bg(t.colors.rail)
            .border_t_1()
            .border_color(t.colors.line)
            .font_family(t.typography.ui_font.clone())
            .text_size(t.typography.small)
            .text_color(t.colors.muted)
            .child(summary)
            .child(keys)
    }
}
