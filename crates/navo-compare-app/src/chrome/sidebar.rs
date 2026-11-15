//! The sessions sidebar (NC66): 280 px expanded, a 56 px icon rail collapsed, toggled by
//! Ctrl+\. The width lerps over the side token through the U12b reduced-motion gate.

use gpui_kit::assets::IconName;
use gpui_kit::{AnimationExt, FocusHandle, KeyBinding, ReadGlobal, img};
use navo_compare_profile::recent::{StartRow, StartRows};
use navo_ui::prelude::*;
use navo_ui::primitives::icon;

use crate::icons::{MaterialIcon, cache::IconCache, icon_id};
use crate::motion;
use crate::theme::SidewiseTheme;
use crate::theme_data::palettes::{DIMS, ModeTokens};

gpui_kit::actions!(sidebar, [ToggleRail]);

/// Key context of the sidebar root; Ctrl+\ is bound inside it.
const CONTEXT: &str = "Sidebar";
/// The mark side both modes draw: the Material pack's 1x bucket.
const MARK: u32 = 16;
/// The mark side as a length.
const MARK_PX: f32 = MARK as f32;

/// Binds Ctrl+\ in the `Sidebar` context; the shell calls it from its own `bind_keys`.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("ctrl-\\", ToggleRail, Some(CONTEXT))]);
}

/// A row's mark: a bundled chrome glyph, or a Material file mark from U11c.
#[derive(Clone, Copy)]
enum Mark {
    Chrome(IconName),
    File(MaterialIcon),
}

/// One entry: its mark, its label and its optional key hint.
struct Entry {
    mark: Mark,
    label: SharedString,
    hint: Option<&'static str>,
}

/// One headed list of entries; an empty heading is the unheaded primary block.
struct Section {
    heading: &'static str,
    entries: Vec<Entry>,
}

/// The sidebar: the rail flag, the width it animates from, and the lists it shows.
pub struct Sidebar {
    rail: bool,
    from: Pixels,
    rows: StartRows,
    tabs: Vec<SharedString>,
    handle: FocusHandle,
}

impl Sidebar {
    /// An expanded sidebar over the stored rows and the open tab titles.
    pub fn new(rows: StartRows, tabs: Vec<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            rail: false,
            from: DIMS.sidebar_w,
            rows,
            tabs,
            handle: cx.focus_handle(),
        }
    }

    /// Whether the icon rail is showing.
    pub fn rail(&self) -> bool {
        self.rail
    }

    /// The width the sidebar settles at: the rail or the full sidebar.
    pub fn width(&self) -> Pixels {
        if self.rail {
            DIMS.rail_w
        } else {
            DIMS.sidebar_w
        }
    }

    /// Ctrl+\: swap rail and sidebar, animating from the width being left.
    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.from = self.width();
        self.rail = !self.rail;
        cx.notify();
    }

    /// The primary actions, then the three lists, in prototype order.
    fn sections(&self) -> Vec<Section> {
        let recent = self
            .rows
            .recent
            .iter()
            .map(|r| entry_of(r, false))
            .collect();
        let named = self.rows.named.iter().map(|r| entry_of(r, true)).collect();
        let tabs = self
            .tabs
            .iter()
            .map(|title| Entry {
                mark: Mark::File(MaterialIcon::Document),
                label: title.clone(),
                hint: None,
            })
            .collect();
        vec![
            Section {
                heading: "",
                entries: primary(),
            },
            Section {
                heading: "Open tabs",
                entries: tabs,
            },
            Section {
                heading: "Recent",
                entries: recent,
            },
            Section {
                heading: "Named",
                entries: named,
            },
        ]
    }

    /// The label and hint of one entry, or nothing in the rail: the rail is icon-only.
    fn label_view(&self, entry: &Entry, m: &ModeTokens) -> Option<Div> {
        if self.rail {
            return None;
        }
        let mut text = div()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .text_color(m.text)
            .child(entry.label.clone());
        if let Some(hint) = entry.hint {
            text = text.child(div().text_color(m.faint).child(hint));
        }
        Some(text)
    }

    /// One row: its mark, then its label only when the sidebar is expanded.
    fn row_view(&self, entry: &Entry, m: &ModeTokens, cx: &mut Context<Self>) -> Div {
        let mut row = div()
            .flex()
            .items_center()
            .gap_2()
            .min_h(DIMS.row_h)
            .child(mark_view(entry.mark, m.muted, cx));
        if let Some(label) = self.label_view(entry, m) {
            row = row.child(label);
        }
        row
    }

    /// One headed list; the rail drops the heading along with the labels.
    fn group_view(&self, section: &Section, m: &ModeTokens, cx: &mut Context<Self>) -> Div {
        let mut group = div().flex().flex_col().gap_1();
        if !section.heading.is_empty() && !self.rail {
            group = group.child(div().text_color(m.muted).child(section.heading));
        }
        group.children(
            section
                .entries
                .iter()
                .map(|entry| self.row_view(entry, m, cx)),
        )
    }

    /// The footer actions, pushed to the bottom and icon-only in the rail.
    fn footer_view(&self, m: &ModeTokens, cx: &mut Context<Self>) -> Div {
        let accounts = Entry {
            mark: Mark::File(MaterialIcon::Lock),
            label: "Accounts".into(),
            hint: None,
        };
        let appearance = Entry {
            mark: Mark::Chrome(IconName::Palette),
            label: "Appearance".into(),
            hint: None,
        };
        div()
            .mt_auto()
            .flex()
            .flex_col()
            .gap_1()
            .child(self.row_view(&accounts, m, cx))
            .child(self.row_view(&appearance, m, cx))
    }
}

/// The two primary actions of the prototype, with their key hints.
#[rustfmt::skip]
fn primary() -> Vec<Entry> {
    vec![
        Entry { mark: Mark::Chrome(IconName::Plus), label: "New comparison".into(), hint: Some("Ctrl+T") },
        Entry { mark: Mark::File(MaterialIcon::Git), label: "Git compare".into(), hint: Some("6") },
    ]
}

/// One stored session as an entry: a named session keeps its name, a recent one its two sides.
fn entry_of(row: &StartRow, named: bool) -> Entry {
    let mark = if named {
        Mark::Chrome(IconName::Star)
    } else {
        Mark::File(icon_id(&row.left, true, false))
    };
    let label = match &row.name {
        Some(name) if named => name.clone().into(),
        _ => format!("{} ↔ {}", row.left, row.right).into(),
    };
    Entry {
        mark,
        label,
        hint: None,
    }
}

/// Draws one mark at the 1x bucket; a missing raster leaves the row without a glyph.
fn mark_view(mark: Mark, color: Hsla, cx: &mut App) -> AnyElement {
    match mark {
        Mark::Chrome(name) => icon(name, px(MARK_PX), color).into_any_element(),
        Mark::File(material) => match IconCache::image(material, MARK, cx) {
            Some(image) => img(image).size(px(MARK_PX)).flex_none().into_any_element(),
            None => div().size(px(MARK_PX)).flex_none().into_any_element(),
        },
    }
}

impl Render for Sidebar {
    /// The sidebar on `--bg2` with a `--sep` hairline, its text kept 16 px off the sides and
    /// 12 px off the top. The width lerps from the previous one, or snaps under reduced motion.
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let side = *SidewiseTheme::global(cx);
        let m = if side.mode.is_dark() {
            side.spec.dark
        } else {
            side.spec.light
        };
        let (from, to) = (self.from, self.width());
        let sections = self.sections();
        let footer = self.footer_view(&m, cx);
        let body = div()
            .key_context(CONTEXT)
            .track_focus(&self.handle)
            .on_action(cx.listener(|this, _: &ToggleRail, _, cx| this.toggle(cx)))
            .flex()
            .flex_col()
            .flex_none()
            .h_full()
            .w(to)
            .overflow_hidden()
            .bg(m.surface2)
            .border_r_1()
            .border_color(m.sep)
            .px(DIMS.inset_x)
            .pt(DIMS.inset_y)
            .gap_3()
            .children(
                sections
                    .iter()
                    .map(|section| self.group_view(section, &m, cx)),
            )
            .child(footer);
        match motion::anim("sidebar-width", side.motion.side_ms, cx) {
            Some(enter) => body
                .with_animation("sidebar-width", enter, move |el, delta| {
                    el.w(from + (to - from) * delta)
                })
                .into_any_element(),
            None => body.into_any_element(),
        }
    }
}

#[cfg(test)]
#[path = "sidebar_tests.rs"]
mod sidebar_tests;
