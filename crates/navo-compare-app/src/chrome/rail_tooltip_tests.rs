//! Unit tests for collapsed-rail tooltip target selection (`A1a`).

use super::*;

#[gpui_kit::test]
fn priority_prefers_row_over_overlay_over_focus_and_falls_back_in_order() {
    let mut tips = RailTips::new();
    let new_cmp = Target::new(RowKey::NewCompare, "New comparison");
    let tab0 = Target::new(RowKey::Tab(0), "Alpha vs Beta");
    let tab0_row = Target::new(RowKey::Tab(0), "Alpha vs Beta (row)");

    assert_eq!(tips.active_target(), None);

    tips.sync_focus(Some(new_cmp.clone()));
    assert_eq!(tips.active_target(), Some(&new_cmp));

    assert!(tips.set_hovered(HoverPlace::Overlay, tab0.clone(), true));
    assert_eq!(tips.active_target(), Some(&tab0));

    assert!(tips.set_hovered(HoverPlace::Row, tab0_row.clone(), true));
    assert_eq!(tips.active_target(), Some(&tab0_row));
    assert!(!tips.set_hovered(HoverPlace::Row, tab0_row.clone(), true));

    assert!(tips.set_hovered(HoverPlace::Row, tab0_row, false));
    assert_eq!(tips.active_target(), Some(&tab0));

    assert!(tips.set_hovered(HoverPlace::Overlay, tab0, false));
    assert_eq!(tips.active_target(), Some(&new_cmp));

    tips.sync_focus(None);
    assert_eq!(tips.active_target(), None);
}

#[gpui_kit::test]
fn out_of_order_hover_exits_clear_only_matching_place_and_key() {
    let mut tips = RailTips::new();
    let tab0 = Target::new(RowKey::Tab(0), "First");
    let tab1 = Target::new(RowKey::Tab(1), "Second");

    assert!(tips.set_hovered(HoverPlace::Row, tab0.clone(), true));
    assert!(tips.set_hovered(HoverPlace::Row, tab1.clone(), true));
    assert!(!tips.set_hovered(HoverPlace::Row, tab0.clone(), false));
    assert_eq!(tips.active_target(), Some(&tab1));

    assert!(!tips.set_hovered(HoverPlace::Overlay, tab1.clone(), true));
    assert!(!tips.set_hovered(HoverPlace::Overlay, tab0, false));
    assert!(!tips.set_hovered(HoverPlace::Row, tab1.clone(), false));
    assert_eq!(tips.active_target(), Some(&tab1));

    assert!(tips.set_hovered(HoverPlace::Overlay, tab1, false));
    assert_eq!(tips.active_target(), None);
}

#[gpui_kit::test]
fn entering_different_row_clears_stale_overlay_target() {
    let mut tips = RailTips::new();
    let focus_row = Target::new(RowKey::NewCompare, "New comparison");
    let tab0 = Target::new(RowKey::Tab(0), "Alpha");
    let tab1 = Target::new(RowKey::Tab(1), "Beta");

    tips.sync_focus(Some(focus_row.clone()));
    assert!(tips.set_hovered(HoverPlace::Overlay, tab0.clone(), true));
    assert_eq!(tips.active_target(), Some(&tab0));

    assert!(tips.set_hovered(HoverPlace::Row, tab1.clone(), true));
    assert_eq!(tips.active_target(), Some(&tab1));

    assert!(tips.set_hovered(HoverPlace::Row, tab1, false));
    assert_eq!(
        tips.active_target(),
        Some(&focus_row),
        "old overlay hover must not resurrect after leaving a different row"
    );
}

#[gpui_kit::test]
fn sync_focus_replaces_label_and_clear_resets_all_targets() {
    let mut tips = RailTips::new();
    let tab0_old = Target::new(RowKey::Tab(0), "Old tab title");
    let tab0_new = Target::new(RowKey::Tab(0), "Updated tab title");
    let tab1 = Target::new(RowKey::Tab(1), "Second");

    tips.sync_focus(Some(tab0_old));
    assert!(tips.set_hovered(HoverPlace::Overlay, tab1.clone(), true));
    assert!(!tips.set_hovered(HoverPlace::Row, tab1.clone(), true));

    tips.sync_focus(Some(tab0_new.clone()));
    assert_eq!(tips.active_target(), Some(&tab1));

    assert!(!tips.set_hovered(HoverPlace::Row, tab1.clone(), false));
    assert!(tips.set_hovered(HoverPlace::Overlay, tab1.clone(), false));
    assert_eq!(tips.active_target(), Some(&tab0_new));

    assert!(tips.set_hovered(HoverPlace::Row, tab1.clone(), true));
    assert!(!tips.set_hovered(HoverPlace::Overlay, tab1, true));
    tips.clear();
    assert_eq!(tips.active_target(), None);

    tips.sync_focus(Some(tab0_new.clone()));
    assert_eq!(tips.active_target(), Some(&tab0_new));
}
