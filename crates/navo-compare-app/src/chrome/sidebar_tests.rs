//! Tests of `Sidebar`: rail collapse toggle, reduced motion, prototype sections, and icon inset.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, Bounds, Point, Render, Subscription, TestAppContext, WindowBounds,
    WindowOptions, div, px, size,
};
use navo_compare_profile::appearance::PaletteId;
use navo_compare_profile::recent::RowKind;
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

/// Host container catching any unconsumed `ToggleRail` action that escapes `Sidebar`.
struct ToggleHost {
    sidebar: Entity<Sidebar>,
    parent_toggled: Rc<Cell<bool>>,
    _sub: Subscription,
}

impl ToggleHost {
    /// Subscribes to `sidebar` events into `events` and stores the host wrapper state.
    fn new(
        sidebar: Entity<Sidebar>,
        events: EventLog,
        parent_toggled: Rc<Cell<bool>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let _sub = cx.subscribe(&sidebar, move |_, _, event: &SidebarEvent, _| {
            events.borrow_mut().push(*event);
        });
        Self {
            sidebar,
            parent_toggled,
            _sub,
        }
    }
}

impl Render for ToggleHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let parent_toggled = self.parent_toggled.clone();
        div()
            .on_action(move |_: &ToggleRail, _, _| parent_toggled.set(true))
            .flex()
            .size_full()
            .child(self.sidebar.clone())
    }
}

/// Opens a themed host window with `Sidebar` subscribed to `events`.
fn host(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Sidebar>, EventLog, Rc<Cell<bool>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let parent_toggled = Rc::new(Cell::new(false));
    let sink = events.clone();
    let escaped = parent_toggled.clone();
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
    let ((window, _host), sidebar) = cx.update(|cx| {
        let mut out = None;
        let win = gpui_kit::open_window(options, cx, |_, cx| {
            let sidebar = cx.new(|cx| Sidebar::new(rows(), vec!["tab".into()], cx));
            out = Some(sidebar.clone());
            cx.new(|cx| ToggleHost::new(sidebar, sink, escaped, cx))
        })
        .expect("open host window");
        (win, out.unwrap())
    });
    cx.update_window(window, |_, window, cx| {
        sidebar.read(cx).handle.clone().focus(window, cx);
        window.render_frame(cx);
    })
    .expect("render");
    (window, sidebar, events, parent_toggled)
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
