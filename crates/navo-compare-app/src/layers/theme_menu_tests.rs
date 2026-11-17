//! Tests of the Theme picker's state (TH4): the start row, palette and mode picks, and silence
//! on a pick that changes nothing.

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::{Subscription, TestAppContext};

use super::*;

/// A picker over `palette` and `mode`, plus the events it emits.
fn rig(
    cx: &mut TestAppContext,
    palette: PaletteId,
    mode: AppearanceMode,
) -> (
    Entity<ThemeMenu>,
    Rc<RefCell<Vec<ThemeMenuEvent>>>,
    Subscription,
) {
    cx.update(gpui_kit::init);
    let menu = cx.new(|cx| ThemeMenu::new(palette, mode, cx));
    let events = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&events);
    let sub = cx.update(|cx| {
        cx.subscribe(&menu, move |_, event: &ThemeMenuEvent, _| {
            log.borrow_mut().push(*event)
        })
    });
    (menu, events, sub)
}

/// A new picker starts on its palette's row.
#[gpui_kit::test]
fn a_new_menu_starts_on_the_current_palette(cx: &mut TestAppContext) {
    let (menu, _events, _sub) = rig(cx, PaletteId::Ocean, AppearanceMode::System);
    assert_eq!(menu.read_with(cx, |m, _| m.active), 2);
}

/// Choosing another palette changes it and emits one `Changed`.
#[gpui_kit::test]
fn choosing_a_palette_changes_it_and_says_so(cx: &mut TestAppContext) {
    let (menu, events, _sub) = rig(cx, PaletteId::Violet, AppearanceMode::System);
    menu.update(cx, |m, cx| m.apply(Pick::Palette(PaletteId::Ember), cx));
    assert_eq!(
        menu.read_with(cx, |m, _| m.choice()),
        (PaletteId::Ember, AppearanceMode::System)
    );
    assert_eq!(*events.borrow(), vec![ThemeMenuEvent::Changed]);
}

/// Choosing a mode leaves the palette alone.
#[gpui_kit::test]
fn choosing_a_mode_keeps_the_palette(cx: &mut TestAppContext) {
    let (menu, events, _sub) = rig(cx, PaletteId::Graphite, AppearanceMode::System);
    menu.update(cx, |m, cx| m.apply(Pick::Mode(AppearanceMode::Dark), cx));
    assert_eq!(
        menu.read_with(cx, |m, _| m.choice()),
        (PaletteId::Graphite, AppearanceMode::Dark)
    );
    assert_eq!(*events.borrow(), vec![ThemeMenuEvent::Changed]);
}

/// Choosing what is already chosen emits nothing.
#[gpui_kit::test]
fn choosing_the_current_value_is_silent(cx: &mut TestAppContext) {
    let (menu, events, _sub) = rig(cx, PaletteId::Ocean, AppearanceMode::Light);
    menu.update(cx, |m, cx| {
        m.apply(Pick::Palette(PaletteId::Ocean), cx);
        m.apply(Pick::Mode(AppearanceMode::Light), cx);
    });
    assert!(events.borrow().is_empty());
}
