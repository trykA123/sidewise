//! The topbar (NC66): a 52 px bar of mode/screen crumbs and action pills above the session.
//! Everything shrinks with `min_w_0` so the bar never clips down to the 780 px minimum width.

use std::rc::Rc;

use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::InputEvent;
use gpui_kit::{FocusHandle, KeyBinding, ReadGlobal, Subscription};
use navo_ui::prelude::*;
use navo_ui::primitives::{TextField, TextFieldState, text_field_state};

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
        // Skip `/` while an `Input` owns focus so users can type path separators in filters.
        KeyBinding::new("/", FocusSearch, Some("Topbar && !Input")),
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

/// The callback fired whenever user typing or clearing edits the filter text.
type OnFilterChange = Rc<dyn Fn(SharedString, &mut App)>;

/// Breadcrumb strings shown on the left side of the topbar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crumbs {
    pub title: SharedString,
    pub detail: SharedString,
}

/// The topbar: its crumbs, its filter text and the callback its actions fire.
pub struct Topbar {
    title: SharedString,
    detail: SharedString,
    filter: SharedString,
    filter_input: Entity<TextFieldState>,
    _filter_sub: Subscription,
    on_action: OnAction,
    on_filter_change: Option<OnFilterChange>,
    handle: FocusHandle,
}

impl Topbar {
    /// A topbar over one crumb pair, firing `on_action` for every pill and action key.
    pub fn new(
        crumbs: Crumbs,
        on_action: impl Fn(Action, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter_input = text_field_state("", window, cx);
        let _filter_sub = cx.subscribe_in(
            &filter_input,
            window,
            |this, input, event: &InputEvent, _, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let text = input.read(cx).value();
                this.on_filter_input(text, cx);
            },
        );
        Self {
            title: crumbs.title,
            detail: crumbs.detail,
            filter: SharedString::default(),
            filter_input,
            _filter_sub,
            on_action: Rc::new(on_action),
            on_filter_change: None,
            handle: cx.focus_handle(),
        }
    }

    /// Registers the callback invoked whenever user editing changes the filter text.
    pub fn set_on_filter_change(&mut self, callback: impl Fn(SharedString, &mut App) + 'static) {
        self.on_filter_change = Some(Rc::new(callback));
    }

    /// The current search filter text.
    pub fn filter(&self) -> &SharedString {
        &self.filter
    }

    /// The focus handle, so the shell can focus the bar before its action keys are used.
    pub fn focus_handle(&self) -> FocusHandle {
        self.handle.clone()
    }

    /// Replaces the search filter text and syncs the editable field.
    pub fn set_filter(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = text.into();
        self.filter = text.clone();
        self.filter_input
            .update(cx, |input, cx| input.set_value(text, window, cx));
        cx.notify();
    }

    /// Syncs `filter` from user input edits and notifies any registered change listener.
    fn on_filter_input(&mut self, text: SharedString, cx: &mut Context<Self>) {
        self.filter = text.clone();
        if let Some(callback) = &self.on_filter_change {
            callback(text, cx);
        }
        cx.notify();
    }

    /// Focuses the inline search field and notifies the caller that Search was activated.
    fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter_input
            .update(cx, |input, cx| input.focus(window, cx));
        self.fire(Action::Search, cx);
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

    /// The inline `/` search filter backed by the kit `TextField` primitive.
    fn search_view(&self, m: &ModeTokens, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .flex_initial()
            .flex_shrink(1.0)
            .w(px(250.))
            .min_w(px(80.))
            .h(px(PILL_H))
            .pl_2()
            .rounded(DIMS.btn_r)
            .bg(m.surface)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.focus_search(window, cx)),
            )
            .child(div().text_color(m.muted).child("/"))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .child(TextField::new(
                        "top-search",
                        &self.filter_input,
                        "Search files",
                    )),
            )
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
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.focus_search(window, cx);
            }))
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
