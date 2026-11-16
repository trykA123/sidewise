//! Tests of the View menu (U16b): presets, the Custom flip, the step-down notice, and the
//! click, key and focus behaviour of the popover.

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{Role, TestAppContext};
use navo_compare_profile::appearance::PaletteId;
use navo_ui::tokens::Mode;

use super::*;
use crate::test_window::open;
use crate::theme::apply_sidewise;

/// A menu in a themed, focused window, plus the events it emits.
fn rig(
    cx: &mut TestAppContext,
    prefs: UiPrefs,
) -> (
    AnyWindowHandle,
    Entity<ViewMenu>,
    Rc<RefCell<Vec<ViewMenuEvent>>>,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        apply_sidewise(PaletteId::Violet, Mode::Light, cx);
    });
    let (window, menu) = open(cx, |_, cx| ViewMenu::new(prefs, cx));
    let events = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&events);
    cx.update_window(window, |_, window, cx| {
        let sub = cx.subscribe(&menu, move |_, event: &ViewMenuEvent, _| {
            log.borrow_mut().push(*event)
        });
        std::mem::forget(sub);
        let handle = menu.read(cx).handle.clone();
        handle.focus(window, cx);
        window.render_frame(cx);
    })
    .expect("window");
    (window, menu, events)
}

/// A bare menu for the state tests.
fn bare(cx: &mut TestAppContext, prefs: UiPrefs) -> Entity<ViewMenu> {
    cx.new(|cx| ViewMenu::new(prefs, cx))
}

/// Simple turns words, syntax and fold off and unifies; Rich turns them back on and splits.
#[gpui_kit::test]
fn simple_and_rich_set_their_toggles_and_layout(cx: &mut TestAppContext) {
    let menu = bare(cx, UiPrefs::default());
    cx.update(|cx| {
        menu.update(cx, |m, cx| m.apply(Act::Preset(ViewPresetPref::Simple), cx));
        let m = menu.read(cx);
        let p = m.prefs();
        assert_eq!(
            (p.words, p.syntax, p.fold_unchanged),
            (false, false, false),
            "Simple turns them off"
        );
        assert_eq!(m.layout(), Layout::Unified, "Simple is unified");
        assert_eq!(p.preset, ViewPresetPref::Simple);
        menu.update(cx, |m, cx| m.apply(Act::Preset(ViewPresetPref::Rich), cx));
        let m = menu.read(cx);
        assert!(
            m.prefs().words && m.prefs().syntax && m.prefs().fold_unchanged,
            "Rich turns them on"
        );
        assert_eq!(m.layout(), Layout::Split, "Rich is side by side");
    });
}

/// Any manual toggle or layout change flips the preset to Custom and keeps the other values.
#[gpui_kit::test]
fn a_manual_change_flips_the_preset_to_custom(cx: &mut TestAppContext) {
    let menu = bare(cx, UiPrefs::default());
    cx.update(|cx| {
        menu.update(cx, |m, cx| m.apply(Act::Toggle(Toggle::Wrap), cx));
        let m = menu.read(cx);
        assert_eq!(
            m.prefs().preset,
            ViewPresetPref::Custom,
            "a toggle makes it Custom"
        );
        assert!(
            m.prefs().wrap_lines && m.prefs().words,
            "only the toggled value changed"
        );
        menu.update(cx, |m, cx| m.apply(Act::Preset(ViewPresetPref::Rich), cx));
        menu.update(cx, |m, cx| m.apply(Act::Layout(Layout::Unified), cx));
        assert_eq!(
            menu.read(cx).prefs().preset,
            ViewPresetPref::Custom,
            "a layout change makes it Custom"
        );
    });
}

/// Best and Text step down to Bytes with the notice when the pair has no text view; Bytes does not.
#[gpui_kit::test]
fn best_steps_down_to_bytes_with_the_notice(cx: &mut TestAppContext) {
    let menu = bare(cx, UiPrefs::default());
    cx.update(|cx| {
        let m = menu.read(cx);
        assert_eq!(
            m.step_down(false),
            Some("No text view for this pair. Alt+V opens it as Bytes.")
        );
        assert_eq!(m.step_down(true), None, "a text pair needs no notice");
        menu.update(cx, |m, cx| m.apply(Act::Kind(ViewModePref::Bytes), cx));
        assert_eq!(
            menu.read(cx).step_down(false),
            None,
            "Bytes is already the fallback"
        );
    });
}

/// Clicking an entry applies it; picking a kind also dismisses the menu.
#[gpui_kit::test]
fn clicks_apply_entries_and_a_kind_dismisses(cx: &mut TestAppContext) {
    let (window, menu, events) = rig(cx, UiPrefs::default());
    cx.update_window(window, |_, window, cx| {
        window.click(("view-item", 9usize), cx)
    })
    .expect("window");
    cx.update(|cx| {
        assert!(
            menu.read(cx).prefs().wrap_lines,
            "the Wrap row toggles wrapping"
        );
        assert_eq!(*events.borrow(), vec![ViewMenuEvent::Changed]);
    });
    cx.update_window(window, |_, window, cx| {
        window.click(("view-item", 13usize), cx)
    })
    .expect("window");
    cx.update_window(window, |_, window, cx| {
        assert_eq!(menu.read(cx).prefs().view_mode, ViewModePref::Bytes);
        assert_eq!(
            events.borrow().last(),
            Some(&ViewMenuEvent::Dismissed),
            "a kind closes the menu"
        );
        window.render_frame(cx);
        assert_eq!(window.find(("view-item", 9usize)).checked(), Some(true));
    })
    .expect("window");
}

/// Arrow keys move the active entry, Enter applies it, Escape dismisses, and focus stays in the menu.
#[gpui_kit::test]
fn keys_move_apply_and_dismiss(cx: &mut TestAppContext) {
    let (window, menu, events) = rig(cx, UiPrefs::default());
    cx.update_window(window, |_, window, cx| {
        menu.update(cx, |m, cx| m.focus(window, cx))
    })
    .expect("window");
    cx.simulate_keystrokes(window, "down down down");
    cx.simulate_keystrokes(window, "enter");
    cx.update(|cx| {
        assert_eq!(
            menu.read(cx).layout(),
            Layout::Unified,
            "Rich starts on item 1; three downs reach the unified layout"
        );
        assert_eq!(menu.read(cx).prefs().preset, ViewPresetPref::Custom);
    });
    cx.simulate_keystrokes(window, "up space");
    cx.update(|cx| {
        assert_eq!(
            menu.read(cx).layout(),
            Layout::Split,
            "Up then Space picks side by side"
        )
    });
    cx.simulate_keystrokes(window, "end");
    cx.update(|cx| assert_eq!(menu.read(cx).active, 13, "End goes to the last entry"));
    cx.simulate_keystrokes(window, "escape");
    cx.update_window(window, |_, window, cx| {
        assert_eq!(
            events.borrow().last(),
            Some(&ViewMenuEvent::Dismissed),
            "Escape dismisses"
        );
        assert!(
            menu.read(cx).handle.is_focused(window),
            "focus stays for the host to restore"
        );
    })
    .expect("window");
}

/// The menu draws in both modes, with and without the entrance, and a non-text pair's notice
/// stays out of the menu's own rows.
#[gpui_kit::test]
fn the_menu_draws_in_both_modes_with_and_without_motion(cx: &mut TestAppContext) {
    let (window, _menu, _events) = rig(cx, UiPrefs::default());
    for mode in [Mode::Dark, Mode::Light] {
        cx.update(|cx| apply_sidewise(PaletteId::Violet, mode, cx));
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            cx.set_reduce_motion(true);
            window.render_frame(cx);
            cx.set_reduce_motion(false);
            assert_eq!(window.find("view-menu").role(), Some(Role::Menu));
        })
        .expect("window");
    }
}
