//! The topbar (NC66): a 52 px bar of mode/screen crumbs and action pills above the session.
//! Everything shrinks with `min_w_0` so the bar never clips down to the 780 px minimum width.

use std::rc::Rc;

use gpui_kit::base::{TestSupportExt as _, TestTextExt as _};
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputEvent};
use gpui_kit::component::{Disableable as _, Sizable as _};
use gpui_kit::{FocusHandle, Hsla, KeyBinding, ReadGlobal, Role, Subscription};
use sidewise_kit::prelude::*;
use sidewise_kit::primitives::{TextFieldState, text_field_state};
use sidewise_kit::tokens;

use crate::theme::SidewiseTheme;
use crate::theme_data::palettes::{DIMS, ModeTokens};

gpui_kit::actions!(topbar, [ViewMenu, SyncPlan, FocusSearch]);

/// Key context of the topbar; its action keys are bound inside it.
const CONTEXT: &str = "Topbar";
/// The topbar height of the prototype.
const TOP_H: f32 = 52.;
/// The narrowest window the topbar must fit without horizontal clipping.
pub const MIN_W: f32 = 780.;
/// Width below which the topbar uses compact spacing and omits pill key hints.
const COMPACT_W: f32 = 640.;
/// Viewport width fallback used before the first prepaint measures the topbar directly.
const COMPACT_VIEWPORT_W: f32 = 920.;
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

#[path = "topbar_menu.rs"]
mod menu;
#[path = "topbar_mode.rs"]
mod mode;

pub use mode::{Action, TopbarMode};
use mode::{PILLS, Pill};

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

/// Connection state shown by the Accounts indicator from caller state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccountConnection {
    #[default]
    Disconnected,
    #[expect(dead_code, reason = "the Accounts pill is hidden in alpha")]
    Connected,
}

/// The topbar: its mode, crumbs, filter text, availability, and callback.
pub struct Topbar {
    mode: TopbarMode,
    enabled: [bool; 6],
    compact: bool,
    title: SharedString,
    detail: SharedString,
    filter: SharedString,
    filter_input: Entity<TextFieldState>,
    _filter_sub: Subscription,
    connection: AccountConnection,
    on_action: OnAction,
    on_filter_change: Option<OnFilterChange>,
    handle: FocusHandle,
    menu: Option<gpui_kit::AnyView>,
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
            mode: TopbarMode::default(),
            enabled: [true; 6],
            compact: window.viewport_size().width < px(COMPACT_VIEWPORT_W),
            title: crumbs.title,
            detail: crumbs.detail,
            filter: SharedString::default(),
            filter_input,
            _filter_sub,
            connection: AccountConnection::Disconnected,
            on_action: Rc::new(on_action),
            on_filter_change: None,
            handle: cx.focus_handle(),
            menu: None,
        }
    }

    /// Switches which screen controls the topbar mounts.
    pub fn set_mode(&mut self, mode: TopbarMode, cx: &mut Context<Self>) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        cx.notify();
    }

    /// The screen mode now mounted; for tests.
    #[cfg(test)]
    pub(crate) fn mode(&self) -> TopbarMode {
        self.mode
    }

    /// Enables or disables one action across pills, shortcut handlers, and search focus.
    pub fn set_available(&mut self, action: Action, available: bool, cx: &mut Context<Self>) {
        self.enabled[action as usize] = available;
        cx.notify();
    }

    /// Updates the displayed breadcrumb title and detail strings.
    pub fn set_crumbs(&mut self, crumbs: Crumbs, cx: &mut Context<Self>) {
        self.title = crumbs.title;
        self.detail = crumbs.detail;
        cx.notify();
    }

    /// Returns whether `action` is both mounted in the current mode and enabled.
    pub fn is_available(&self, action: Action) -> bool {
        self.mode.shows(action) && self.enabled[action as usize]
    }

    /// Updates compact mode from observed own width, notifying only on a transition.
    fn record_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        let next = width < px(COMPACT_W);
        if self.compact != next {
            self.compact = next;
            crate::frame_notify::notify_after_frame(cx);
        }
    }

    /// Updates the Accounts connection indicator state from the caller.
    #[expect(dead_code, reason = "the Accounts pill is hidden in alpha")]
    pub fn set_connection(&mut self, state: AccountConnection, cx: &mut Context<Self>) {
        self.connection = state;
        cx.notify();
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

    /// Focuses the inline search field and notifies the caller when Search is available.
    fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_available(Action::Search) {
            return;
        }
        self.filter_input
            .update(cx, |input, cx| input.focus(window, cx));
        self.fire(Action::Search, cx);
    }

    /// Runs the registered callback for one action when it is shown and enabled.
    fn fire(&self, action: Action, cx: &mut App) {
        if !self.is_available(action) {
            return;
        }
        (self.on_action)(action, cx);
    }

    /// Builds one single-line ellipsized breadcrumb span bounded inside its flex column.
    fn crumb_part(text: SharedString, color: Hsla) -> impl IntoElement {
        div()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_color(color)
            .child(text.clone().test_text(text))
    }

    /// The mode/screen crumbs: single-line ellipsized title (plus detail at normal width).
    fn crumb_view(&self, m: &ModeTokens) -> impl IntoElement {
        let label: SharedString = if self.detail.is_empty() {
            self.title.clone()
        } else {
            format!("{} {}", self.title, self.detail).into()
        };
        div()
            .id("top-crumbs")
            .role(Role::Label)
            .aria_label(label)
            .test_support()
            .flex()
            .items_center()
            .gap_1()
            .min_w_0()
            .overflow_hidden()
            .child(Self::crumb_part(self.title.clone(), m.text))
            .when(!self.compact && !self.detail.is_empty(), |crumbs| {
                crumbs.child(Self::crumb_part(self.detail.clone(), m.muted))
            })
    }

    /// The inline `/` search filter backed by the kit `Input` primitive with disabled support.
    fn search_view(&self, m: &ModeTokens, cx: &mut Context<Self>) -> AnyElement {
        let tokens = tokens::current(cx);
        let input = Input::new(&self.filter_input)
            .id("top-search")
            .aria_label("Search files")
            .small()
            .font_family(tokens.typography.mono_font.clone())
            .text_size(tokens.typography.small)
            .disabled(!self.is_available(Action::Search));
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
            .child(div().text_color(m.muted).child("/".test_text("/")))
            .child(div().flex_1().min_w_0().overflow_hidden().child(input))
            .into_any_element()
    }

    /// The optional leading swatch or status dot for the Theme and Accounts buttons.
    fn pill_indicator(&self, action: Action, m: &ModeTokens) -> Option<AnyElement> {
        match action {
            Action::Theme => Some(
                div()
                    .id("top-theme-swatch")
                    .w(px(8.))
                    .h(px(8.))
                    .rounded_full()
                    .flex_none()
                    .bg(m.accent)
                    .border_1()
                    .border_color(m.sep_strong)
                    .test_support()
                    .into_any_element(),
            ),
            Action::Accounts => {
                let (color, status) = match self.connection {
                    AccountConnection::Connected => (m.accent, "Account connected"),
                    AccountConnection::Disconnected => (m.faint, "Account disconnected"),
                };
                Some(
                    div()
                        .id("top-accounts-status")
                        .role(Role::Status)
                        .aria_label(status)
                        .aria_description(status)
                        .w(px(8.))
                        .h(px(8.))
                        .rounded_full()
                        .flex_none()
                        .bg(color)
                        .test_support()
                        .into_any_element(),
                )
            }
            _ => None,
        }
    }

    /// One action pill as a kit button backed by the Sidewise theme tokens.
    fn pill_view(&self, pill: &Pill, m: &ModeTokens, cx: &mut Context<Self>) -> AnyElement {
        let action = pill.action;
        let enabled = self.is_available(action);
        Button::new(pill.id)
            .small()
            .disabled(!enabled)
            .accessibility_label(pill.label)
            .px_2()
            .h(px(PILL_H))
            .rounded(DIMS.btn_r)
            .on_click(cx.listener(move |this, _, _, cx| this.fire(action, cx)))
            .when_some(self.pill_indicator(action, m), |row, indicator| {
                row.child(indicator)
            })
            .child(pill.label.test_text(pill.label))
            .when_some(
                pill.hint.filter(|_| enabled && !self.compact),
                |row, hint| row.child(div().text_color(m.faint).child(hint.test_text(hint))),
            )
            .into_any_element()
    }
    /// Returns a zero-hit overlay canvas that measures own bar width without taking flex space.
    fn width_probe(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.weak_entity();
        gpui_kit::canvas(
            move |bounds, _, cx| {
                let _ = this.update(cx, |bar, cx| bar.record_width(bounds.size.width, cx));
            },
            |_, (), _, _| {},
        )
        .absolute()
        .inset_0()
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
        let search = self
            .mode
            .shows(Action::Search)
            .then(|| self.search_view(&m, cx));
        let pills: Vec<AnyElement> = PILLS
            .iter()
            .filter(|pill| self.mode.shows(pill.action))
            .map(|pill| self.with_menu(pill.action, self.pill_view(pill, &m, cx)))
            .collect();
        div()
            .relative()
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
            .when(!self.compact, |bar| bar.gap_3())
            .when(self.compact, |bar| bar.gap_1())
            .h(px(TOP_H))
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .bg(m.surface2)
            .border_b_1()
            .border_color(m.sep)
            .px(DIMS.inset_x)
            .child(self.width_probe(cx))
            .child(crumb)
            .child(div().flex_1().min_w_0())
            .when_some(search, |bar, search| bar.child(search))
            .children(pills)
    }
}
