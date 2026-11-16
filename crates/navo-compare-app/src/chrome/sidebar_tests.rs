//! Tests of `Sidebar`: rail collapse toggle, reduced motion, prototype sections, and icon inset.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, Bounds, FocusHandle, Focusable as _, Modifiers, Point, Render, Role,
    Subscription, TestAppContext, VisualTestContext, WindowBounds, WindowOptions, div, point, px,
    size,
};
use navo_compare_profile::appearance::PaletteId;
use navo_compare_profile::recent::RowKind;
use navo_ui::primitives::{TextField, TextFieldState, text_field_state};
use navo_ui::tokens::Mode;

use super::*;
use crate::test_window::open;
use crate::theme::apply_sidewise;

/// One recent and one named session, so every list has a row.
fn rows() -> StartRows {
    let row = StartRow {
        id: "1".into(),
        name: None,
        left: "left".into(),
        right: "right".into(),
        kind: RowKind::Paths,
        age: "now".into(),
    };
    StartRows {
        recent: vec![row.clone()],
        named: vec![StartRow {
            name: Some("Saved".into()),
            ..row
        }],
    }
}

/// The active mode's tokens, as the render reads them.
fn mode_tokens(cx: &App) -> ModeTokens {
    let side = SidewiseTheme::global(cx);
    if side.mode.is_dark() {
        side.spec.dark
    } else {
        side.spec.light
    }
}

/// A focused sidebar in a themed window; the tokens exist before the first frame.
fn sidebar(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Sidebar>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        apply_sidewise(PaletteId::Violet, Mode::Light, cx);
        bind_keys(cx);
    });
    let (window, view) = open(cx, |_, cx| Sidebar::new(rows(), vec!["tab".into()], cx));
    cx.update_window(window, |_, window, cx| {
        let handle = view.read(cx).handle.clone();
        handle.focus(window, cx);
        window.render_frame(cx);
    })
    .expect("window");
    (window, view)
}

/// Ctrl+\ collapses 280 -> 56 and drops the label bounds; a second press restores them.
#[gpui_kit::test]
fn ctrl_backslash_toggles_the_rail_and_the_labels(cx: &mut TestAppContext) {
    let (window, view) = sidebar(cx);
    cx.update(|cx| {
        let expanded = view.read(cx);
        assert_eq!(expanded.width(), DIMS.sidebar_w, "starts expanded at 280px");
        assert!(!expanded.rail(), "starts as the full sidebar");
        let entry = &expanded.sections()[1].entries[0];
        assert!(
            expanded.label_view(entry, &mode_tokens(cx)).is_some(),
            "the expanded sidebar shows labels"
        );
    });
    cx.simulate_keystrokes(window, "ctrl-\\");
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let rail = view.read(cx);
        assert!(rail.rail(), "Ctrl+\\ must collapse to the rail");
        assert_eq!(rail.width(), DIMS.rail_w, "the rail is 56px");
        let entry = &rail.sections()[1].entries[0];
        assert!(
            rail.label_view(entry, &mode_tokens(cx)).is_none(),
            "the rail omits label bounds"
        );
    })
    .expect("window");
    cx.simulate_keystrokes(window, "ctrl-\\");
    cx.update(|cx| {
        assert!(!view.read(cx).rail(), "a second press expands again");
        assert_eq!(view.read(cx).width(), DIMS.sidebar_w, "back to 280px");
    });
}

/// Under reduced motion the width snaps to the rail and no animation is scheduled.
#[gpui_kit::test]
fn reduced_motion_snaps_the_width(cx: &mut TestAppContext) {
    let (window, view) = sidebar(cx);
    cx.update(|cx| cx.set_reduce_motion(true));
    cx.simulate_keystrokes(window, "ctrl-\\");
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            view.read(cx).width(),
            DIMS.rail_w,
            "the width snaps, not lerps"
        );
        assert!(
            motion::anim("sidebar", SidewiseTheme::global(cx).motion.side_ms, cx).is_none(),
            "reduced motion must stop the collapse animation"
        );
    })
    .expect("window");
}

/// The sections and the two primary actions match `sidewise-chosen`.
#[gpui_kit::test]
fn the_sections_match_the_prototype(cx: &mut TestAppContext) {
    let (_window, view) = sidebar(cx);
    cx.update(|cx| {
        let sections = view.read(cx).sections();
        let headings: Vec<_> = sections.iter().map(|s| s.heading).collect();
        assert_eq!(
            headings,
            ["", "Open tabs", "Recent", "Named"],
            "section order"
        );
        assert_eq!(sections[0].entries.len(), 2, "two primary actions");
        assert_eq!(
            sections[0].entries[0].label,
            SharedString::from("New comparison")
        );
        assert_eq!(
            sections[0].entries[1].label,
            SharedString::from("Git compare")
        );
        assert_eq!(sections[2].entries.len(), 1, "the recent list");
        assert_eq!(sections[3].entries.len(), 1, "the named list");
    });
}

/// Stop-if probe: the rail's icon fits between the side insets, and text keeps the owner's
/// minimum 16 px side and 12 px top inset.
#[test]
fn the_rail_icon_fits_and_the_text_insets_hold() {
    let content = f32::from(DIMS.rail_w) - 2.0 * f32::from(DIMS.inset_x);
    assert!(
        content >= MARK_PX,
        "icon-only rows would overlap at 56px: {content}px for a {MARK}px mark"
    );
    assert!(
        f32::from(DIMS.inset_x) >= 16.0,
        "text needs 16px off the sides"
    );
    assert!(
        f32::from(DIMS.inset_y) >= 12.0,
        "text needs 12px off the top"
    );
}

/// Shared log of events emitted by `Sidebar`.
type EventLog = Rc<RefCell<Vec<SidebarEvent>>>;

/// Host container catching any unconsumed `ToggleRail` action or keystroke escaping `Sidebar`.
struct ToggleHost {
    sidebar: Entity<Sidebar>,
    parent_toggled: Rc<Cell<bool>>,
    parent_keys: Rc<Cell<usize>>,
    outside: FocusHandle,
    input: Entity<TextFieldState>,
    _sub: Subscription,
}

/// Opened host test rig exposing `Sidebar` state, events, escaped parent counters, and focus targets.
struct HostRig {
    window: AnyWindowHandle,
    view: Entity<Sidebar>,
    events: EventLog,
    parent_toggled: Rc<Cell<bool>>,
    parent_keys: Rc<Cell<usize>>,
    outside: FocusHandle,
    input: Entity<TextFieldState>,
}

impl ToggleHost {
    /// Subscribes to `sidebar` events into `events` and allocates escaped-action and focus state.
    fn new(
        sidebar: Entity<Sidebar>,
        events: EventLog,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let _sub = cx.subscribe(&sidebar, move |_, _, event: &SidebarEvent, _| {
            events.borrow_mut().push(*event);
        });
        Self {
            sidebar,
            parent_toggled: Rc::new(Cell::new(false)),
            parent_keys: Rc::new(Cell::new(0)),
            outside: cx.focus_handle().tab_stop(true),
            input: text_field_state("", window, cx),
            _sub,
        }
    }
}

impl Render for ToggleHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let parent_toggled = self.parent_toggled.clone();
        let parent_keys = self.parent_keys.clone();
        div()
            .on_action(move |_: &ToggleRail, _, _| parent_toggled.set(true))
            .on_key_down(move |_, _, _| parent_keys.set(parent_keys.get() + 1))
            .flex()
            .size_full()
            .child(self.sidebar.clone())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(TextField::new("host-input", &self.input, "Filter"))
                    .child(div().id("host-outside").track_focus(&self.outside).flex_1()),
            )
    }
}

/// Opens a themed host window with `Sidebar` subscribed to `events`.
fn open_host_with_outside(cx: &mut TestAppContext) -> HostRig {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    cx.update(|cx| {
        gpui_kit::init(cx);
        apply_sidewise(PaletteId::Violet, Mode::Light, cx);
        bind_keys(cx);
    });
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Point::default(),
            size: size(px(800.), px(600.)),
        })),
        ..Default::default()
    };
    let ((window, host), view) = cx.update(|cx| {
        let mut out = None;
        let win = gpui_kit::open_window(options, cx, |window, cx| {
            let sidebar = cx.new(|cx| Sidebar::new(rows(), vec!["tab".into()], cx));
            out = Some(sidebar.clone());
            cx.new(|cx| ToggleHost::new(sidebar, sink, window, cx))
        })
        .expect("open host window");
        (win, out.unwrap())
    });
    let (parent_toggled, parent_keys, outside, input) = cx.update(|cx| {
        let h = host.read(cx);
        (
            h.parent_toggled.clone(),
            h.parent_keys.clone(),
            h.outside.clone(),
            h.input.clone(),
        )
    });
    cx.update_window(window, |_, window, cx| {
        view.read(cx).handle.clone().focus(window, cx);
        window.render_frame(cx);
    })
    .expect("render");
    HostRig {
        window,
        view,
        events,
        parent_toggled,
        parent_keys,
        outside,
        input,
    }
}

/// Opens a themed host window with `Sidebar` subscribed to `events`.
fn host(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Sidebar>, EventLog, Rc<Cell<bool>>) {
    let rig = open_host_with_outside(cx);
    (rig.window, rig.view, rig.events, rig.parent_toggled)
}

/// Toggling emits `RailToggled(bool)` for both transitions and consumes `ToggleRail` propagation.
#[gpui_kit::test]
fn toggle_rail_emits_transitions_and_stops_action_propagation(cx: &mut TestAppContext) {
    let (window, view, events, parent_toggled) = host(cx);
    cx.simulate_keystrokes(window, "ctrl-\\");
    assert_eq!(*events.borrow(), vec![SidebarEvent::RailToggled(true)]);
    assert!(
        !parent_toggled.get(),
        "Sidebar must stop ToggleRail bubbling"
    );
    assert!(view.read_with(cx, |sb, _| sb.rail()));

    cx.simulate_keystrokes(window, "ctrl-\\");
    assert_eq!(
        *events.borrow(),
        vec![
            SidebarEvent::RailToggled(true),
            SidebarEvent::RailToggled(false)
        ]
    );
    assert!(
        !parent_toggled.get(),
        "Sidebar must stop ToggleRail bubbling"
    );
    assert!(!view.read_with(cx, |sb, _| sb.rail()));

    events.borrow_mut().clear();
    view.update(cx, |sb, cx| sb.toggle_rail(cx));
    assert_eq!(*events.borrow(), vec![SidebarEvent::RailToggled(true)]);
}

/// `set_tabs` updates section labels, clamps out-of-range and empty `active`, and preserves handles.
#[gpui_kit::test]
fn set_tabs_updates_labels_clamps_active_and_reuses_handles(cx: &mut TestAppContext) {
    let (_window, view, _events, _parent) = host(cx);
    view.update(cx, |sb, cx| {
        assert_eq!(sb.active, 0);
        assert_eq!(sb.tab_focuses.len(), 1);
        let h0 = sb.tab_focuses[0].clone();

        sb.set_tabs(
            vec!["First".into(), "Second".into(), "Third".into()],
            99,
            cx,
        );
        assert_eq!(sb.active, 2);
        assert_eq!(sb.tab_focuses.len(), 3);
        assert_eq!(sb.tab_focuses[0], h0);
        let tab_labels: Vec<_> = sb.sections()[1]
            .entries
            .iter()
            .map(|e| e.label.clone())
            .collect();
        assert_eq!(
            tab_labels,
            vec![
                SharedString::from("First"),
                SharedString::from("Second"),
                SharedString::from("Third"),
            ]
        );
        let h1 = sb.tab_focuses[1].clone();

        sb.set_tabs(vec!["One".into(), "Two".into()], 5, cx);
        assert_eq!(sb.active, 1);
        assert_eq!(sb.tab_focuses.len(), 2);
        assert_eq!(sb.tab_focuses[0], h0);
        assert_eq!(sb.tab_focuses[1], h1);

        sb.set_tabs(vec![], 4, cx);
        assert_eq!(sb.active, 0);
        assert!(sb.tab_focuses.is_empty());
        assert!(sb.sections()[1].entries.is_empty());
    });
}

/// Verifies non-tab/non-primary rows stay non-button/non-focused while open tabs expose `TabList`/`Tab`.
fn assert_static_rows_and_rail_labels(rig: &HostRig, cx: &mut TestAppContext) {
    cx.update_window(rig.window, |_, window, cx| {
        let tabs = window.find("tabs");
        assert_eq!(tabs.role(), Some(Role::TabList));
        assert_eq!(tabs.label(), Some("Open tabs"));
        let tab0 = window.find(("open-tab", 0usize));
        assert_eq!(tab0.role(), Some(Role::Tab));
        assert_eq!(tab0.selected(), Some(true));
        for label in [
            "Git compare",
            "left ↔ right",
            "Saved",
            "Accounts",
            "Appearance",
        ] {
            let snap = window.find_by_label(label);
            assert_ne!(snap.role(), Some(Role::Button));
            assert_ne!(snap.role(), Some(Role::Tab));
            assert_ne!(snap.focused(), Some(true));
        }
        assert!(rig.view.read(cx).sections()[0].entries[1].hint.is_none());

        rig.view.update(cx, |sb, cx| sb.toggle_rail(cx));
        window.render_frame(cx);
        assert_eq!(window.find("tabs").role(), Some(Role::TabList));
        for label in [
            "New comparison",
            "Git compare",
            "tab",
            "left ↔ right",
            "Saved",
            "Accounts",
            "Appearance",
        ] {
            assert!(window.try_find_by_label(label).is_some(), "{label} in rail");
        }
    })
    .expect("static check");
}

/// Mounted `New comparison` row focuses on mousedown and `focus_primary`, emits `NewCompare`
/// once on click or unmodified Enter/Space, propagates modified/unrelated keys, and leaves static rows non-button.
#[gpui_kit::test]
fn primary_row_mounts_focus_emits_new_compare_and_keeps_static_rows_static(
    cx: &mut TestAppContext,
) {
    let rig = open_host_with_outside(cx);
    cx.update_window(rig.window, |_, window, cx| {
        rig.outside.focus(window, cx);
        window.render_frame(cx);
        assert!(!rig.view.read(cx).contains_focus(window, cx));

        rig.view.update(cx, |sb, cx| sb.focus_primary(window, cx));
        window.render_frame(cx);
        assert!(rig.view.read(cx).contains_focus(window, cx));
        let btn = window.find("new-compare");
        assert_eq!(btn.role(), Some(Role::Button));
        assert_eq!(btn.label(), Some("New comparison"));
        assert_eq!(btn.focused(), Some(true));

        rig.outside.focus(window, cx);
        window.render_frame(cx);
        assert!(!rig.view.read(cx).contains_focus(window, cx));
        window.click("new-compare", cx);
        window.render_frame(cx);
        assert!(rig.view.read(cx).contains_focus(window, cx));
        assert_eq!(window.find("new-compare").focused(), Some(true));

        let before = rig.parent_keys.get();
        window.press("enter", cx);
        window.press("space", cx);
        assert_eq!(rig.parent_keys.get(), before, "Enter/Space must consume");

        window.press("ctrl-enter", cx);
        window.press("shift-space", cx);
        window.press("a", cx);
        assert_eq!(rig.parent_keys.get(), before + 3);
    })
    .expect("window");
    assert_eq!(*rig.events.borrow(), vec![SidebarEvent::NewCompare; 3]);
    assert_static_rows_and_rail_labels(&rig, cx);
}

/// Asserts `TabList`/`Tab` roles, `selected` flags, and visible labels match `expected` and `active`.
fn assert_tab_selection(window: &gpui_kit::Window, expected: &[&str], active: usize) {
    let list = window.find("tabs");
    assert_eq!(list.role(), Some(Role::TabList));
    assert_eq!(list.label(), Some("Open tabs"));
    for (i, label) in expected.iter().enumerate() {
        let tab = window.find(("open-tab", i));
        assert_eq!(tab.role(), Some(Role::Tab));
        assert_eq!(tab.label(), Some(*label));
        assert_eq!(tab.selected(), Some(i == active), "tab {i} selected");
    }
}

/// Open tabs expose `TabList`/`Tab`, emit `SelectTab` without mutating `active`, and hide close in rail.
#[gpui_kit::test]
fn tab_row_emits_select_tab_once_preserves_external_active_and_renders_selection(
    cx: &mut TestAppContext,
) {
    let rig = open_host_with_outside(cx);
    cx.update_window(rig.window, |_, window, cx| {
        rig.view.update(cx, |sb, cx| {
            sb.set_tabs(vec!["A".into(), "B".into(), "C".into()], 1, cx)
        });
        window.render_frame(cx);
        assert_tab_selection(window, &["A", "B", "C"], 1);

        window.click(("open-tab", 0usize), cx);
        window.render_frame(cx);
        assert_eq!(window.find(("open-tab", 0usize)).focused(), Some(true));
        assert_tab_selection(window, &["A", "B", "C"], 1);

        let before = rig.parent_keys.get();
        window.press("enter", cx);
        window.press("space", cx);
        assert_eq!(rig.parent_keys.get(), before, "Enter/Space must consume");
        window.press("ctrl-enter", cx);
        window.press("shift-space", cx);
        window.press("a", cx);
        assert_eq!(rig.parent_keys.get(), before + 3);
    })
    .expect("tab select");
    assert_eq!(*rig.events.borrow(), vec![SidebarEvent::SelectTab(0); 3]);

    cx.update_window(rig.window, |_, window, cx| {
        rig.view.update(cx, |sb, cx| {
            sb.set_tabs(vec!["X".into(), "Y".into()], 9, cx)
        });
        window.render_frame(cx);
        assert_tab_selection(window, &["X", "Y"], 1);

        rig.view.update(cx, |sb, cx| sb.toggle_rail(cx));
        window.render_frame(cx);
        assert!(
            window.try_find(("tab-close", 0usize)).is_none(),
            "rail hides close"
        );
        assert_tab_selection(window, &["X", "Y"], 1);

        rig.view.update(cx, |sb, cx| sb.set_tabs(vec![], 4, cx));
        window.render_frame(cx);
        assert_eq!(window.find("tabs").role(), Some(Role::TabList));
        assert!(window.try_find(("open-tab", 0usize)).is_none());
    })
    .expect("clamped and empty tabs");
}

/// Close child meets >=24px hit size and emits only `CloseTab` while preserving focused `TextField`.
#[gpui_kit::test]
fn tab_close_emits_close_tab_only_with_size_and_keeps_textfield_focus(cx: &mut TestAppContext) {
    let rig = open_host_with_outside(cx);
    cx.update_window(rig.window, |_, window, cx| {
        rig.view.update(cx, |sb, cx| {
            sb.set_tabs(vec!["First".into(), "Second".into()], 0, cx)
        });
        window.render_frame(cx);

        let close = window.find(("tab-close", 1usize));
        assert_eq!(close.role(), Some(Role::Button));
        assert_eq!(close.label(), Some("Close tab"));
        assert_ne!(close.focused(), Some(true));
        let bounds = close.bounds();
        assert!(bounds.size.width >= px(24.) && bounds.size.height >= px(24.));

        rig.input.update(cx, |input, cx| input.focus(window, cx));
        window.render_frame(cx);
        assert!(rig.input.read(cx).focus_handle(cx).is_focused(window));
        assert!(!rig.view.read(cx).contains_focus(window, cx));

        window.click(("tab-close", 1usize), cx);
        window.render_frame(cx);
        assert!(rig.input.read(cx).focus_handle(cx).is_focused(window));
        assert!(!rig.view.read(cx).contains_focus(window, cx));
    })
    .expect("close tab");
    assert_eq!(*rig.events.borrow(), vec![SidebarEvent::CloseTab(1)]);
}

/// Sends `key` in `window` and asserts `target` gains focus.
fn assert_focus_step(
    window: &mut gpui_kit::Window,
    key: &str,
    target: impl Into<gpui_kit::ElementId>,
    cx: &mut App,
) {
    window.press(key, cx);
    window.render_frame(cx);
    assert_eq!(window.find(target).focused(), Some(true), "{key}");
}

/// Checks root-entry and wrapping traversal with empty and updated tabs, propagation, and Enter/Space.
fn assert_empty_and_grown_navigation(rig: &HostRig, cx: &mut TestAppContext) {
    cx.update_window(rig.window, |_, window, cx| {
        rig.view.update(cx, |sb, cx| sb.set_tabs(vec![], 0, cx));
        rig.view.read(cx).handle.clone().focus(window, cx);
        window.render_frame(cx);
        assert_focus_step(window, "up", "new-compare", cx);
        assert_focus_step(window, "tab", "new-compare", cx);

        rig.view.update(cx, |sb, cx| {
            sb.set_tabs(vec!["First".into(), "Second".into()], 0, cx)
        });
        window.render_frame(cx);
        assert_focus_step(window, "down", ("open-tab", 0usize), cx);
        assert_focus_step(window, "tab", ("open-tab", 1usize), cx);
        assert_focus_step(window, "tab", "new-compare", cx);
        assert_focus_step(window, "shift-tab", ("open-tab", 1usize), cx);
        assert_focus_step(window, "up", ("open-tab", 0usize), cx);
        assert!(rig.events.borrow().is_empty(), "focus move emits nothing");

        let before = rig.parent_keys.get();
        window.press("enter", cx);
        for key in ["ctrl-down", "shift-up", "ctrl-tab", "alt-tab", "a"] {
            window.press(key, cx);
        }
        assert_eq!(rig.parent_keys.get(), before + 5);
    })
    .expect("empty and grown navigation");
    assert_eq!(*rig.events.borrow(), vec![SidebarEvent::SelectTab(0)]);
}

/// Down/Up and Tab/Shift+Tab cycle `New comparison` and `Open tabs`, skip static rows, and wrap cleanly.
#[gpui_kit::test]
fn navigation_cycles_wrapped_rows_including_root_empty_and_set_tabs(cx: &mut TestAppContext) {
    let rig = open_host_with_outside(cx);
    cx.update_window(rig.window, |_, window, cx| {
        assert_focus_step(window, "down", "new-compare", cx);
        assert_focus_step(window, "down", ("open-tab", 0usize), cx);
        assert_focus_step(window, "down", "new-compare", cx);
        assert_focus_step(window, "up", ("open-tab", 0usize), cx);

        rig.view.read(cx).handle.clone().focus(window, cx);
        window.render_frame(cx);
        assert_focus_step(window, "shift-tab", ("open-tab", 0usize), cx);
    })
    .expect("initial navigation");
    assert_empty_and_grown_navigation(&rig, cx);
}

/// Pressing `Tab` stays in `Sidebar` when focused there, and keeps normal `Root` traversal on an outside `TextField`.
#[gpui_kit::test]
fn outside_textfield_tab_keeps_root_traversal_outside_sidebar(cx: &mut TestAppContext) {
    let rig = open_host_with_outside(cx);
    cx.update_window(rig.window, |_, window, cx| {
        rig.view.update(cx, |sb, cx| {
            sb.set_tabs(vec!["First".into(), "Second".into()], 0, cx)
        });
        window.render_frame(cx);
        rig.view.read(cx).tab_focuses[1].clone().focus(window, cx);
        window.render_frame(cx);
        assert_focus_step(window, "tab", "new-compare", cx);

        rig.input.update(cx, |input, cx| input.focus(window, cx));
        window.render_frame(cx);
        assert!(rig.input.read(cx).focus_handle(cx).is_focused(window));
        assert!(!rig.view.read(cx).contains_focus(window, cx));

        window.press("tab", cx);
        window.render_frame(cx);
        assert!(
            rig.outside.is_focused(window),
            "Tab must use Root traversal"
        );
        assert!(!rig.view.read(cx).contains_focus(window, cx));

        rig.input.update(cx, |input, cx| input.focus(window, cx));
        window.render_frame(cx);
        window.press("down", cx);
        window.render_frame(cx);
        assert!(!rig.view.read(cx).contains_focus(window, cx));
    })
    .expect("outside textfield tab");
    assert!(rig.events.borrow().is_empty());
}

/// Opens the host in rail mode with the first frames settled; returns the rig.
fn rail_rig(cx: &mut TestAppContext) -> HostRig {
    let rig = open_host_with_outside(cx);
    rig.view.update(cx, |sb, cx| sb.toggle_rail(cx));
    cx.update_window(rig.window, |_, window, cx| window.render_frame(cx))
        .expect("window");
    rig
}

/// Moves the pointer to `pt` with no modifiers.
fn move_mouse(cx: &mut TestAppContext, window: AnyWindowHandle, pt: Point<Pixels>) {
    VisualTestContext::from_window(window, cx).simulate_mouse_move(pt, None, Modifiers::none());
}

/// The centre of the control `id`.
fn center_of(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    id: impl Into<ElementId> + 'static,
) -> Point<Pixels> {
    cx.update_window(window, |_, window, _| window.find(id).bounds().center())
        .expect("window")
}

/// Renders bounded frames until `"rail-tooltip"` presence matches `want` (no fixed sleep).
fn settle_tip(cx: &mut TestAppContext, window: AnyWindowHandle, want: bool) {
    for _ in 0..6 {
        cx.run_until_parked();
        let matched = cx
            .update_window(window, |_, window, cx| {
                window.render_frame(cx);
                window.try_find("rail-tooltip").is_some() == want
            })
            .expect("window");
        if matched {
            return;
        }
    }
    cx.update_window(window, |_, window, _| {
        assert_eq!(
            window.try_find("rail-tooltip").is_some(),
            want,
            "tooltip did not settle"
        );
    })
    .expect("window");
}

/// The visible tooltip's label, if any.
fn tip_label(cx: &mut TestAppContext, window: AnyWindowHandle) -> Option<String> {
    cx.update_window(window, |_, window, _| {
        window
            .try_find("rail-tooltip")
            .and_then(|t| t.label().map(String::from))
    })
    .expect("window")
}

/// Rail hover shows the tip 10 px right of the outer row for both ring widths; static rows stay silent.
#[gpui_kit::test]
fn rail_hover_shows_the_tip_beside_the_outer_row_and_static_rows_stay_silent(
    cx: &mut TestAppContext,
) {
    let rig = rail_rig(cx);
    for (palette, mode, ring) in [
        (PaletteId::Violet, Mode::Light, 2.0_f32),
        (PaletteId::Contrast, Mode::Dark, 3.0_f32),
    ] {
        cx.update(|cx| apply_sidewise(palette, mode, cx));
        assert_eq!(cx.update(|cx| SidewiseTheme::global(cx).spec.ring_px), ring);
        let rows: [(ElementId, &str); 2] = [
            ("new-compare".into(), "New comparison"),
            (("open-tab", 0usize).into(), "tab"),
        ];
        for (id, label) in rows {
            let pt = center_of(cx, rig.window, id.clone());
            move_mouse(cx, rig.window, pt);
            settle_tip(cx, rig.window, true);
            cx.update_window(rig.window, |_, window, _| {
                let row = window.find(id.clone()).bounds();
                let tip = window.find("rail-tooltip");
                assert_eq!(tip.label(), Some(label));
                assert_eq!(tip.role(), Some(Role::Tooltip));
                assert_eq!(
                    tip.bounds().origin.x,
                    row.right() + px(10.),
                    "{id:?} {ring}px"
                );
                assert_eq!(tip.bounds().origin.y, row.origin.y);
            })
            .expect("window");
        }
        let recent = center_of(cx, rig.window, ("Recent", 0usize));
        move_mouse(cx, rig.window, recent);
        settle_tip(cx, rig.window, false);
    }
}

/// Keyboard focus shows the tip, row hover beats it, and leaving the row falls back to focus.
#[gpui_kit::test]
fn focus_shows_the_tip_and_row_hover_beats_it(cx: &mut TestAppContext) {
    let rig = rail_rig(cx);
    cx.update_window(rig.window, |_, window, cx| {
        rig.view.update(cx, |sb, cx| sb.focus_primary(window, cx));
    })
    .expect("window");
    settle_tip(cx, rig.window, true);
    assert_eq!(tip_label(cx, rig.window).as_deref(), Some("New comparison"));

    let tab = center_of(cx, rig.window, ("open-tab", 0usize));
    move_mouse(cx, rig.window, tab);
    settle_tip(cx, rig.window, true);
    assert_eq!(tip_label(cx, rig.window).as_deref(), Some("tab"));

    move_mouse(cx, rig.window, point(px(700.), px(500.)));
    settle_tip(cx, rig.window, true);
    assert_eq!(tip_label(cx, rig.window).as_deref(), Some("New comparison"));
}

/// Escape consumes while a tip shows, propagates once it is gone, and stays latched under a resting
/// pointer and focus until every source for that row leaves.
#[gpui_kit::test]
fn escape_dismisses_the_tip_and_latches_until_all_sources_leave(cx: &mut TestAppContext) {
    let rig = rail_rig(cx);
    assert!(
        !rig.view.update(cx, |sb, cx| sb.dismiss_tip(cx)),
        "no tip: nothing to dismiss"
    );
    cx.update_window(rig.window, |_, window, cx| {
        rig.view.update(cx, |sb, cx| sb.focus_primary(window, cx));
    })
    .expect("window");
    let row = center_of(cx, rig.window, "new-compare");
    move_mouse(cx, rig.window, row);
    settle_tip(cx, rig.window, true);

    let before = rig.parent_keys.get();
    cx.simulate_keystrokes(rig.window, "escape");
    settle_tip(cx, rig.window, false);
    assert_eq!(
        rig.parent_keys.get(),
        before,
        "a dismissing Escape is consumed"
    );
    cx.simulate_keystrokes(rig.window, "escape");
    assert_eq!(
        rig.parent_keys.get(),
        before + 1,
        "an idle Escape propagates"
    );
    move_mouse(cx, rig.window, point(px(700.), px(500.)));
    settle_tip(cx, rig.window, false);
    move_mouse(cx, rig.window, row);
    settle_tip(cx, rig.window, false);

    move_mouse(cx, rig.window, point(px(700.), px(500.)));
    cx.update_window(rig.window, |_, window, cx| rig.outside.focus(window, cx))
        .expect("window");
    move_mouse(cx, rig.window, row);
    settle_tip(cx, rig.window, true);
}

/// Escape while the pointer is on the tip itself removes it and keeps it gone under the resting pointer.
#[gpui_kit::test]
fn escape_on_the_hovered_overlay_stays_dismissed_under_a_resting_pointer(cx: &mut TestAppContext) {
    let rig = rail_rig(cx);
    let row = center_of(cx, rig.window, ("open-tab", 0usize));
    move_mouse(cx, rig.window, row);
    settle_tip(cx, rig.window, true);
    let tip = center_of(cx, rig.window, "rail-tooltip");
    move_mouse(cx, rig.window, tip);
    settle_tip(cx, rig.window, true);
    assert_eq!(tip_label(cx, rig.window).as_deref(), Some("tab"));

    let before = rig.parent_keys.get();
    cx.simulate_keystrokes(rig.window, "escape");
    settle_tip(cx, rig.window, false);
    assert_eq!(
        rig.parent_keys.get(),
        before,
        "the dismissing Escape is consumed"
    );
    move_mouse(cx, rig.window, tip);
    for _ in 0..3 {
        settle_tip(cx, rig.window, false);
    }
    cx.update_window(rig.window, |_, window, _| {
        assert!(
            window.try_find("rail-tooltip-bridge").is_none(),
            "no ghost bridge"
        );
    })
    .expect("window");

    move_mouse(cx, rig.window, point(px(700.), px(500.)));
    move_mouse(cx, rig.window, row);
    settle_tip(cx, rig.window, true);
}

/// Expanding the rail, relabelling a tab and removing a tab each drop a tip that was showing.
#[gpui_kit::test]
fn expansion_and_tab_changes_drop_the_tip(cx: &mut TestAppContext) {
    let rig = rail_rig(cx);
    let tab = center_of(cx, rig.window, ("open-tab", 0usize));
    move_mouse(cx, rig.window, tab);
    settle_tip(cx, rig.window, true);
    rig.view
        .update(cx, |sb, cx| sb.set_tabs(vec!["renamed".into()], 0, cx));
    settle_tip(cx, rig.window, false);

    rig.view.update(cx, |sb, cx| {
        sb.set_tabs(vec!["a".into(), "b".into()], 0, cx)
    });
    cx.update_window(rig.window, |_, window, cx| window.render_frame(cx))
        .expect("window");
    let second = center_of(cx, rig.window, ("open-tab", 1usize));
    move_mouse(cx, rig.window, second);
    settle_tip(cx, rig.window, true);
    rig.view
        .update(cx, |sb, cx| sb.set_tabs(vec!["a".into()], 0, cx));
    settle_tip(cx, rig.window, false);

    let first = center_of(cx, rig.window, "new-compare");
    move_mouse(cx, rig.window, first);
    settle_tip(cx, rig.window, true);
    rig.view.update(cx, |sb, cx| sb.toggle_rail(cx));
    settle_tip(cx, rig.window, false);
    let expanded = center_of(cx, rig.window, "new-compare");
    move_mouse(cx, rig.window, expanded);
    settle_tip(cx, rig.window, false);
}
