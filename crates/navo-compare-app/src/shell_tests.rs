//! View tests of the shell: tab rules (NS11), keys, focus and closing the last tab.

use gpui_kit::base::test_support::snapshots;
use gpui_kit::component::theme::{Theme as KitTheme, ThemeMode};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, EntityId, Modifiers, Point, ReadGlobal,
    Role, TestAppContext, VisualTestContext, WindowAppearance, WindowBounds, WindowOptions, px,
    size,
};
use navo_compare_profile::appearance::PaletteId;
use navo_ui::tokens::{self, Mode};

use crate::shell::{Shell, bind_keys};
use crate::start::Side;
use crate::test_window::open;
use crate::theme::{SidewiseTheme, apply_sidewise, mode_of};
use crate::theme_data::palettes::{PALETTES, PaletteSpec};

/// Opens a shell in a test window and renders one frame.
fn shell(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Shell>) {
    let (window, view) = open(cx, Shell::new);
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .expect("window");
    (window, view)
}

/// Runs `f` on the shell inside its window and renders the next frame.
fn act(
    cx: &mut TestAppContext,
    (window, view): &(AnyWindowHandle, Entity<Shell>),
    f: impl FnOnce(&mut Shell, &mut gpui_kit::Window, &mut gpui_kit::Context<Shell>),
) {
    cx.update_window(*window, |_, window, cx| {
        view.update(cx, |shell, cx| f(shell, window, cx));
        window.render_frame(cx);
    })
    .expect("window");
}

/// Returns (tab count, active index) of the shell.
fn state(cx: &mut TestAppContext, view: &Entity<Shell>) -> (usize, usize) {
    view.read_with(cx, |shell, _| (shell.tab_count(), shell.active()))
}

/// The start screen entity ids of all tabs, in order.
fn ids(cx: &mut TestAppContext, view: &Entity<Shell>) -> Vec<EntityId> {
    view.read_with(cx, |shell, _| shell.tab_ids())
}

/// The entity id of the active tab's start screen.
fn active_id(cx: &mut TestAppContext, view: &Entity<Shell>) -> EntityId {
    view.read_with(cx, |shell, _| {
        shell.active_screen().map(|screen| screen.entity_id())
    })
    .expect("active start screen")
}

#[gpui_kit::test]
fn one_start_tab_at_launch(cx: &mut TestAppContext) {
    let (_, view) = shell(cx);
    assert_eq!(state(cx, &view), (1, 0));
}

#[gpui_kit::test]
fn new_tabs_go_to_the_end_and_become_active(cx: &mut TestAppContext) {
    let s = shell(cx);
    act(cx, &s, |shell, window, cx| shell.new_tab(window, cx));
    act(cx, &s, |shell, window, cx| shell.new_tab(window, cx));
    assert_eq!(state(cx, &s.1), (3, 2));
}

#[gpui_kit::test]
fn closing_the_middle_tab_activates_its_right_neighbour(cx: &mut TestAppContext) {
    let s = shell(cx);
    act(cx, &s, |shell, window, cx| {
        shell.new_tab(window, cx);
        shell.new_tab(window, cx);
        shell.select_tab(1, window, cx);
    });
    let before = ids(cx, &s.1);
    act(cx, &s, |shell, window, cx| shell.close_tab(1, window, cx));
    assert_eq!(state(cx, &s.1), (2, 1));
    assert_eq!(ids(cx, &s.1), [before[0], before[2]]);
    assert_eq!(active_id(cx, &s.1), before[2]);
}

#[gpui_kit::test]
fn closing_the_last_index_activates_the_new_last(cx: &mut TestAppContext) {
    let s = shell(cx);
    act(cx, &s, |shell, window, cx| {
        shell.new_tab(window, cx);
        shell.new_tab(window, cx);
    });
    let before = ids(cx, &s.1);
    act(cx, &s, |shell, window, cx| shell.close_tab(2, window, cx));
    assert_eq!(state(cx, &s.1), (2, 1));
    assert_eq!(ids(cx, &s.1), [before[0], before[1]]);
    assert_eq!(active_id(cx, &s.1), before[1]);
}

#[gpui_kit::test]
fn closing_a_tab_left_of_the_active_one_keeps_the_active_tab(cx: &mut TestAppContext) {
    let s = shell(cx);
    act(cx, &s, |shell, window, cx| {
        shell.new_tab(window, cx);
        shell.new_tab(window, cx);
    });
    let before = ids(cx, &s.1);
    act(cx, &s, |shell, window, cx| shell.close_tab(0, window, cx));
    assert_eq!(state(cx, &s.1), (2, 1));
    assert_eq!(ids(cx, &s.1), [before[1], before[2]]);
    assert_eq!(active_id(cx, &s.1), before[2]);
}

#[gpui_kit::test]
fn closing_an_inactive_tab_keeps_the_focus_in_the_right_field(cx: &mut TestAppContext) {
    let s = shell(cx);
    act(cx, &s, |shell, window, cx| shell.new_tab(window, cx));
    cx.simulate_keystrokes(s.0, "tab");
    let side = |cx: &mut TestAppContext| {
        cx.update_window(s.0, |_, window, cx| {
            s.1.read(cx)
                .active_screen()
                .and_then(|screen| screen.read(cx).focused_side(window, cx))
        })
        .expect("window")
    };
    assert_eq!(side(cx), Some(Side::Right));
    act(cx, &s, |shell, window, cx| shell.close_tab(0, window, cx));
    assert_eq!(state(cx, &s.1), (1, 0));
    assert_eq!(side(cx), Some(Side::Right));
}

#[gpui_kit::test]
fn next_and_previous_wrap(cx: &mut TestAppContext) {
    let s = shell(cx);
    act(cx, &s, |shell, window, cx| {
        shell.new_tab(window, cx);
        shell.new_tab(window, cx);
        shell.next_tab(window, cx);
    });
    assert_eq!(state(cx, &s.1), (3, 0));
    act(cx, &s, |shell, window, cx| shell.prev_tab(window, cx));
    assert_eq!(state(cx, &s.1), (3, 2));
}

#[gpui_kit::test]
fn an_index_out_of_range_changes_nothing(cx: &mut TestAppContext) {
    let s = shell(cx);
    act(cx, &s, |shell, window, cx| {
        shell.new_tab(window, cx);
        shell.select_tab(9, window, cx);
        shell.close_tab(9, window, cx);
    });
    assert_eq!(state(cx, &s.1), (2, 1));
    assert_eq!(cx.windows().len(), 1);
}

#[gpui_kit::test]
fn ctrl_t_adds_a_tab_and_ctrl_w_closes_one(cx: &mut TestAppContext) {
    let s = shell(cx);
    cx.simulate_keystrokes(s.0, "ctrl-t");
    assert_eq!(state(cx, &s.1), (2, 1));
    cx.simulate_keystrokes(s.0, "ctrl-w");
    assert_eq!(state(cx, &s.1), (1, 0));
}

#[gpui_kit::test]
fn ctrl_tab_and_ctrl_shift_tab_move_between_tabs(cx: &mut TestAppContext) {
    let s = shell(cx);
    cx.simulate_keystrokes(s.0, "ctrl-t");
    cx.simulate_keystrokes(s.0, "ctrl-tab");
    assert_eq!(state(cx, &s.1), (2, 0));
    cx.simulate_keystrokes(s.0, "ctrl-shift-tab");
    assert_eq!(state(cx, &s.1), (2, 1));
}

#[gpui_kit::test]
fn clicking_a_tab_selects_it_and_its_close_button_closes_it(cx: &mut TestAppContext) {
    let s = shell(cx);
    act(cx, &s, |shell, window, cx| shell.new_tab(window, cx));
    cx.update_window(s.0, |_, window, cx| {
        window.within("tabs").click(0usize, cx);
    })
    .expect("window");
    assert_eq!(state(cx, &s.1), (2, 0));
    cx.update_window(s.0, |_, window, cx| {
        window.within("tabs").click(("tab-close", 0usize), cx);
    })
    .expect("window");
    assert_eq!(state(cx, &s.1), (1, 0));
}

#[gpui_kit::test]
fn the_first_tab_has_a_width(cx: &mut TestAppContext) {
    let s = shell(cx);
    let width = cx
        .update_window(s.0, |_, window, _| {
            window.within("tabs").find(0usize).bounds().size.width
        })
        .expect("window");
    assert!(f32::from(width) > 0.);
}

#[gpui_kit::test]
fn the_new_tabs_left_field_has_focus(cx: &mut TestAppContext) {
    let s = shell(cx);
    act(cx, &s, |shell, window, cx| shell.new_tab(window, cx));
    let side = cx
        .update_window(s.0, |_, window, cx| {
            s.1.read(cx)
                .active_screen()
                .and_then(|screen| screen.read(cx).focused_side(window, cx))
        })
        .expect("window");
    assert_eq!(side, Some(Side::Left));
}

#[gpui_kit::test]
fn closing_the_only_tab_closes_the_window(cx: &mut TestAppContext) {
    let s = shell(cx);
    act(cx, &s, |shell, window, cx| shell.close_tab(0, window, cx));
    cx.run_until_parked();
    assert_eq!(cx.windows().len(), 0);
}

/// Finds the static specification for `palette` by id.
fn palette_spec(palette: PaletteId) -> PaletteSpec {
    PALETTES
        .iter()
        .find(|(id, _)| *id == palette)
        .map(|(_, spec)| *spec)
        .expect("palette spec")
}

/// Asserts that `SidewiseTheme`, navo tokens, and `KitTheme` all reflect `palette` and `mode`.
fn assert_theme_projection(cx: &App, palette: PaletteId, mode: Mode) {
    let side = SidewiseTheme::global(cx);
    assert_eq!(side.palette, palette, "Sidewise palette");
    assert_eq!(side.mode, mode, "Sidewise mode");
    let spec = palette_spec(palette);
    let m = if mode.is_dark() {
        spec.dark
    } else {
        spec.light
    };
    let navo = tokens::current(cx);
    assert_eq!(navo.mode, mode, "navo mode");
    assert_eq!(navo.colors.background, m.surface, "navo background");
    assert_eq!(navo.colors.surface, m.card, "navo surface");
    assert_eq!(navo.colors.text, m.text, "navo text");
    assert_eq!(navo.colors.accent, m.accent, "navo accent");
    let kit_mode = if mode.is_dark() {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    let kit = KitTheme::global(cx);
    assert_eq!(kit.mode, kit_mode, "kit mode");
    assert_eq!(kit.colors.background, m.surface, "kit background");
    assert_eq!(kit.colors.foreground, m.text, "kit foreground");
    assert_eq!(kit.colors.primary, m.accent, "kit primary");
    assert_eq!(
        kit.tokens.background.color, m.surface,
        "kit token background"
    );
    assert_eq!(kit.tokens.accent.color, m.accent_tint, "kit token accent");
}

/// Opening `Shell` after `apply_sidewise(Ocean)` retains Ocean and keeps projections coherent
/// across direct `Shell::apply_appearance` calls for Dark and Light appearances.
#[gpui_kit::test]
fn shell_retains_ocean_palette_on_open_and_appearance_updates(cx: &mut TestAppContext) {
    let (window, _) = cx.update(|cx| {
        gpui_kit::init(cx);
        apply_sidewise(PaletteId::Ocean, Mode::Light, cx);
        bind_keys(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::default(),
                size: size(px(800.), px(600.)),
            })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Shell::new(window, cx))
        })
        .expect("open test window")
    });
    let open_mode = cx
        .update_window(window, |_, window, _| mode_of(window.appearance()))
        .expect("window");
    cx.update(|cx| {
        assert_theme_projection(cx, PaletteId::Ocean, open_mode);
        Shell::apply_appearance(WindowAppearance::Dark, cx);
        assert_theme_projection(cx, PaletteId::Ocean, Mode::Dark);
        Shell::apply_appearance(WindowAppearance::Light, cx);
        assert_theme_projection(cx, PaletteId::Ocean, Mode::Light);
    });
}

/// Opening `Shell` with no prior `SidewiseTheme` global defaults to `PaletteId::Violet`.
#[gpui_kit::test]
fn shell_defaults_to_violet_without_prior_sidewise_global(cx: &mut TestAppContext) {
    let (window, _) = shell(cx);
    let open_mode = cx
        .update_window(window, |_, window, _| mode_of(window.appearance()))
        .expect("window");
    cx.update(|cx| {
        assert_theme_projection(cx, PaletteId::Violet, open_mode);
        Shell::apply_appearance(WindowAppearance::Dark, cx);
        assert_theme_projection(cx, PaletteId::Violet, Mode::Dark);
    });
}

/// Opens a sized, themed `Shell` window in Light or Dark mode and renders one frame.
fn shell_at(cx: &mut TestAppContext, width: f32, mode: Mode) -> (AnyWindowHandle, Entity<Shell>) {
    let (window, view) = cx.update(|cx| {
        gpui_kit::init(cx);
        tokens::apply(mode, cx);
        bind_keys(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::default(),
                size: size(px(width), px(600.)),
            })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Shell::new(window, cx))
        })
        .expect("open test window")
    });
    if mode.is_dark() {
        cx.update(|cx| Shell::apply_appearance(WindowAppearance::Dark, cx));
    }
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .expect("window");
    (window, view)
}

/// Clicks the center of an element by id and advances the test executor.
fn click_id(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) {
    let center = cx
        .update_window(window, |_, window, _| window.find(id).bounds().center())
        .expect("window");
    VisualTestContext::from_window(window, cx).simulate_click(center, Modifiers::none());
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .expect("window");
}

/// Start-mode Topbar shows only disabled right-side pills, hides Search/Sync, and renders cleanly
/// at 780 and 1200 px in Light and Dark without fake placeholder dialogs.
#[gpui_kit::test]
fn topbar_in_start_mode_mounts_disabled_pills_without_search_or_sync(cx: &mut TestAppContext) {
    for mode in [Mode::Light, Mode::Dark] {
        for width in [780., 1200.] {
            let (window, view) = shell_at(cx, width, mode);
            cx.update_window(window, |_, window, _| {
                assert!(window.try_find("top-search").is_none());
                assert!(window.try_find("top-sync").is_none());
                for id in ["top-theme", "top-accounts", "top-view", "top-keys"] {
                    let pill = window.find(id);
                    assert!(pill.visible(), "{id} visible at {width}px {mode:?}");
                    assert_eq!(
                        pill.disabled(),
                        Some(true),
                        "{id} disabled at {width}px {mode:?}"
                    );
                    let b = pill.bounds();
                    assert!(
                        b.origin.x >= px(0.) && b.origin.x + b.size.width <= px(width),
                        "{id} fits {width}px"
                    );
                }
                for snap in snapshots(window) {
                    if let Some(label) = snap.label() {
                        assert!(!label.contains("Coming soon"), "no placeholder dialogs");
                    }
                }
            })
            .expect("window");

            for id in ["top-theme", "top-accounts", "top-view", "top-keys"] {
                click_id(cx, window, id);
                assert_eq!(
                    state(cx, &view),
                    (1, 0),
                    "clicking disabled {id} is a no-op"
                );
            }
        }
    }
}

/// StatusBar renders `Role::Status`, derived singular/plural summary, and only the two Shell hints.
#[gpui_kit::test]
fn status_bar_tracks_derived_summary_singular_plural_and_two_hints(cx: &mut TestAppContext) {
    for mode in [Mode::Light, Mode::Dark] {
        for width in [780., 1200.] {
            let s = shell_at(cx, width, mode);
            cx.update_window(s.0, |_, window, _| {
                let bar = window.find("status-bar");
                assert_eq!(bar.role(), Some(Role::Status));
                assert!(bar.bounds().origin.x + bar.bounds().size.width <= px(width));
                assert_eq!(
                    window.find("status-summary").label(),
                    Some("New compare · 1 open tab")
                );
                for id in ["hint-new", "hint-close"] {
                    assert!(window.find(id).path().starts_with(bar.path()));
                }
                assert!(window.try_find("hint-sidebar").is_none());
            })
            .expect("window");

            act(cx, &s, |shell, window, cx| shell.new_tab(window, cx));
            cx.update_window(s.0, |_, window, _| {
                assert_eq!(
                    window.find("status-summary").label(),
                    Some("New compare · 2 open tabs")
                );
            })
            .expect("window");

            act(cx, &s, |shell, window, cx| shell.close_tab(1, window, cx));
            cx.update_window(s.0, |_, window, _| {
                assert_eq!(
                    window.find("status-summary").label(),
                    Some("New compare · 1 open tab")
                );
            })
            .expect("window");
        }
    }
}

/// Closing an inactive tab updates the status summary count while keeping `Side::Right` focus.
#[gpui_kit::test]
fn closing_inactive_tab_updates_status_summary_while_keeping_right_focus(cx: &mut TestAppContext) {
    let s = shell(cx);
    act(cx, &s, |shell, window, cx| shell.new_tab(window, cx));
    cx.simulate_keystrokes(s.0, "tab");
    cx.update_window(s.0, |_, window, cx| {
        assert_eq!(
            window.find("status-summary").label(),
            Some("New compare · 2 open tabs")
        );
        let side =
            s.1.read(cx)
                .active_screen()
                .and_then(|screen| screen.read(cx).focused_side(window, cx));
        assert_eq!(side, Some(Side::Right));
    })
    .expect("window");

    act(cx, &s, |shell, window, cx| shell.close_tab(0, window, cx));
    cx.update_window(s.0, |_, window, cx| {
        assert_eq!(
            window.find("status-summary").label(),
            Some("New compare · 1 open tab")
        );
        let side =
            s.1.read(cx)
                .active_screen()
                .and_then(|screen| screen.read(cx).focused_side(window, cx));
        assert_eq!(side, Some(Side::Right));
    })
    .expect("window");
}

/// Bottom-right toast overlay sits above the status bar without intercepting clicks on Start inputs.
#[gpui_kit::test]
fn toast_slot_mounts_bottom_right_without_blocking_start_fields(cx: &mut TestAppContext) {
    for mode in [Mode::Light, Mode::Dark] {
        for width in [780., 1200.] {
            let (window, view) = shell_at(cx, width, mode);
            cx.update_window(window, |_, window, _| {
                let slot = window.find("toast-slot").bounds();
                let status = window.find("status-bar").bounds();
                let left = window.find("start-left").bounds();
                let right = window.find("start-right").bounds();
                assert!(slot.origin.x > px(width / 2.), "bottom-right slot x");
                assert!(
                    slot.origin.y + slot.size.height <= status.origin.y,
                    "above status bar"
                );
                assert!(
                    slot.size.width < px(width / 2.) && slot.origin.y > left.bottom(),
                    "toast slot does not cover Start inputs"
                );
                assert!(right.size.width > px(0.));
            })
            .expect("window");

            click_id(cx, window, "start-right");
            cx.update_window(window, |_, window, cx| {
                let side = view
                    .read(cx)
                    .active_screen()
                    .and_then(|screen| screen.read(cx).focused_side(window, cx));
                assert_eq!(
                    side,
                    Some(Side::Right),
                    "clicking Right input must focus it"
                );
            })
            .expect("window");

            click_id(cx, window, "start-left");
            cx.update_window(window, |_, window, cx| {
                let side = view
                    .read(cx)
                    .active_screen()
                    .and_then(|screen| screen.read(cx).focused_side(window, cx));
                assert_eq!(side, Some(Side::Left), "clicking Left input must focus it");
            })
            .expect("window");
        }
    }
}
