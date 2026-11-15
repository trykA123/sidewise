//! The topbar (NC66): a 52 px bar of mode/screen crumbs and action pills above the session.
//! Everything shrinks with `min_w_0` so the bar never clips down to the 780 px minimum width.

use std::rc::Rc;

use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::Button;
use gpui_kit::{FocusHandle, KeyBinding, ReadGlobal};
use navo_ui::prelude::*;

use crate::theme::SidewiseTheme;
use crate::theme_data::palettes::{DIMS, ModeTokens};

gpui_kit::actions!(topbar, [ViewMenu, SyncPlan, FocusSearch]);

/// Key context of the topbar; its action keys are bound inside it.
const CONTEXT: &str = "Topbar";
/// The topbar height of the prototype.
const TOP_H: f32 = 52.;
/// The narrowest window the topbar must fit without horizontal clipping.
pub const MIN_W: f32 = 780.;
/// The height of one action pill.
const PILL_H: f32 = 28.;

/// Binds the topbar's action keys in the `Topbar` context.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("alt-v", ViewMenu, Some(CONTEXT)),
        KeyBinding::new("alt-s", SyncPlan, Some(CONTEXT)),
        KeyBinding::new("/", FocusSearch, Some(CONTEXT)),
    ]);
}

/// Which topbar action fired; the shell registers one callback for all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Search,
    Theme,
    Accounts,
    View,
    Keys,
    Sync,
}

/// One action pill: its element id, its label, its key hint and the action it fires.
struct Pill {
    id: &'static str,
    label: &'static str,
    hint: Option<&'static str>,
    action: Action,
}

/// The action pills, in prototype order after the search filter.
#[rustfmt::skip]
const PILLS: [Pill; 5] = [
    Pill { id: "top-theme", label: "Theme", hint: Some("T"), action: Action::Theme },
    Pill { id: "top-accounts", label: "Accounts", hint: None, action: Action::Accounts },
    Pill { id: "top-view", label: "View", hint: Some("Alt+V"), action: Action::View },
    Pill { id: "top-keys", label: "Keys", hint: Some("?"), action: Action::Keys },
    Pill { id: "top-sync", label: "Sync", hint: Some("Alt+S"), action: Action::Sync },
];

/// The callback every topbar action fires, on the UI thread.
type OnAction = Rc<dyn Fn(Action, &mut App)>;

/// The topbar: its crumbs, its filter text and the callback its actions fire.
pub struct Topbar {
    title: SharedString,
    detail: SharedString,
    filter: SharedString,
    on_action: OnAction,
    handle: FocusHandle,
}

impl Topbar {
    /// A topbar over one crumb pair, firing `on_action` for every pill and action key.
    pub fn new(
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        on_action: impl Fn(Action, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            filter: SharedString::default(),
            on_action: Rc::new(on_action),
            handle: cx.focus_handle(),
        }
    }

    /// The current search filter text.
    pub fn filter(&self) -> &SharedString {
        &self.filter
    }

    /// The focus handle, so the shell can focus the bar before its action keys are used.
    pub fn focus_handle(&self) -> FocusHandle {
        self.handle.clone()
    }

    /// Replaces the search filter text.
    pub fn set_filter(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.filter = text.into();
        cx.notify();
    }

    /// Runs the registered callback for one action.
    fn fire(&self, action: Action, cx: &mut App) {
        (self.on_action)(action, cx);
    }

    /// The mode/screen crumbs: the bold title, then the muted detail.
    fn crumb_view(&self, m: &ModeTokens) -> Div {
        div()
            .flex()
            .items_center()
            .gap_1()
            .min_w_0()
            .overflow_hidden()
            .child(div().text_color(m.text).child(self.title.clone()))
            .child(div().text_color(m.muted).child(self.detail.clone()))
    }

    /// The inline `/` search filter pill as an accessible, keyboard-focusable button.
    fn search_view(&self, m: &ModeTokens, cx: &mut Context<Self>) -> AnyElement {
        Button::new("top-search")
            .small()
            .accessibility_label("Search files")
            .flex_shrink(1.0)
            .min_w_0()
            .overflow_hidden()
            .px_2()
            .h(px(PILL_H))
            .rounded(DIMS.btn_r)
            .bg(m.surface)
            .text_color(m.muted)
            .on_click(cx.listener(|this, _, _, cx| this.fire(Action::Search, cx)))
            .child("/")
            .child(div().min_w_0().overflow_hidden().child(self.filter.clone()))
            .into_any_element()
    }

    /// One action pill as a kit button backed by the Sidewise theme tokens.
    fn pill_view(&self, pill: &Pill, m: &ModeTokens, cx: &mut Context<Self>) -> AnyElement {
        let action = pill.action;
        Button::new(pill.id)
            .small()
            .accessibility_label(pill.label)
            .px_2()
            .h(px(PILL_H))
            .rounded(DIMS.btn_r)
            .on_click(cx.listener(move |this, _, _, cx| this.fire(action, cx)))
            .child(pill.label)
            .when_some(pill.hint, |row, hint| {
                row.child(div().text_color(m.faint).child(hint))
            })
            .into_any_element()
    }
}

impl Render for Topbar {
    /// The 52 px bar on `--bg2` with a `--sep` hairline, its text 16 px off the sides. The
    /// crumbs and the filter take the shrinking space; the pills keep their size.
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let side = *SidewiseTheme::global(cx);
        let m = if side.mode.is_dark() {
            side.spec.dark
        } else {
            side.spec.light
        };
        let crumb = self.crumb_view(&m);
        let search = self.search_view(&m, cx);
        let pills: Vec<AnyElement> = PILLS
            .iter()
            .map(|pill| self.pill_view(pill, &m, cx))
            .collect();
        div()
            .key_context(CONTEXT)
            .tab_group()
            .track_focus(&self.handle)
            .on_action(cx.listener(|this, _: &ViewMenu, _, cx| this.fire(Action::View, cx)))
            .on_action(cx.listener(|this, _: &SyncPlan, _, cx| this.fire(Action::Sync, cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, _, cx| this.fire(Action::Search, cx)))
            .flex()
            .flex_none()
            .items_center()
            .gap_3()
            .h(px(TOP_H))
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .bg(m.surface2)
            .border_b_1()
            .border_color(m.sep)
            .px(DIMS.inset_x)
            .child(crumb)
            .child(div().flex_1().min_w_0())
            .child(search)
            .children(pills)
    }
}
