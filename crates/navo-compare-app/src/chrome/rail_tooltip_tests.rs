//! Unit tests for collapsed-rail tooltip selection (`A1a`) and suppression/anchor lifecycle (`A1b`).

use gpui_kit::{Bounds, point, px, size};

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

#[gpui_kit::test]
fn dismiss_clears_only_after_both_hover_and_focus_leave_target() {
    let mut tips = RailTips::new();
    let tab0 = Target::new(RowKey::Tab(0), "Alpha");
    let tab1 = Target::new(RowKey::Tab(1), "Beta");
    let tab2 = Target::new(RowKey::Tab(2), "Gamma");

    // Exit order 1: hover exits before focus blurs; re-hover stays suppressed until focus leaves.
    tips.sync_focus(Some(tab0.clone()));
    assert!(!tips.set_hovered(HoverPlace::Overlay, tab0.clone(), true));
    assert!(!tips.set_hovered(HoverPlace::Row, tab0.clone(), true));
    assert!(tips.dismiss());
    assert!(!tips.dismiss());
    assert_eq!(tips.active_target(), None);

    assert!(!tips.set_hovered(HoverPlace::Row, tab0.clone(), false));
    assert_eq!(tips.active_target(), None);
    assert!(!tips.set_hovered(HoverPlace::Row, tab0.clone(), true));
    assert_eq!(tips.active_target(), None);
    assert!(!tips.set_hovered(HoverPlace::Row, tab0.clone(), false));
    tips.sync_focus(None);
    assert!(tips.set_hovered(HoverPlace::Row, tab0.clone(), true));
    assert_eq!(tips.active_target(), Some(&tab0));
    assert!(tips.set_hovered(HoverPlace::Row, tab0, false));

    // Exit order 2: focus blurs before hover exits; re-focus stays suppressed until hover leaves.
    tips.sync_focus(Some(tab1.clone()));
    assert!(!tips.set_hovered(HoverPlace::Row, tab1.clone(), true));
    assert!(tips.dismiss());
    tips.sync_focus(None);
    assert_eq!(tips.active_target(), None);
    tips.sync_focus(Some(tab1.clone()));
    assert_eq!(tips.active_target(), None);

    // Different target can still display once suppressed row-hover ends and exposes focused `tab2`.
    tips.sync_focus(Some(tab2.clone()));
    assert!(tips.set_hovered(HoverPlace::Row, tab1.clone(), false));
    assert_eq!(tips.active_target(), Some(&tab2));

    tips.sync_focus(Some(tab1.clone()));
    assert_eq!(tips.active_target(), Some(&tab1));
}

#[gpui_kit::test]
fn popup_only_hover_lifecycle_prevents_ghost_on_dismiss_retarget_and_sync_tabs() {
    let mut tips = RailTips::new();
    let new_cmp = Target::new(RowKey::NewCompare, "New comparison");
    let tab0 = Target::new(RowKey::Tab(0), "Alpha");
    let tab1 = Target::new(RowKey::Tab(1), "Beta");

    // Popup-only dismiss drops overlay_hover and clears orphan latch immediately.
    assert!(tips.set_hovered(HoverPlace::Overlay, tab0.clone(), true));
    assert!(tips.dismiss());
    assert_eq!(tips.active_target(), None);
    assert!(tips.set_hovered(HoverPlace::Row, tab0.clone(), true));
    assert_eq!(tips.active_target(), Some(&tab0));
    assert!(tips.set_hovered(HoverPlace::Row, tab0.clone(), false));

    // Live popup wins over focus; dismissing popup-only falls back cleanly to focused NewCompare.
    tips.sync_focus(Some(new_cmp.clone()));
    assert!(tips.set_hovered(HoverPlace::Overlay, tab0.clone(), true));
    assert_eq!(tips.active_target(), Some(&tab0));
    assert!(tips.dismiss());
    assert_eq!(tips.active_target(), Some(&new_cmp));
    assert!(tips.set_hovered(HoverPlace::Row, tab0.clone(), true));
    assert_eq!(tips.active_target(), Some(&tab0));
    assert!(tips.set_hovered(HoverPlace::Row, tab0.clone(), false));

    // Unmounted popup after `sync_tabs` cannot linger without `on_hover(false)`.
    tips.sync_focus(Some(tab1.clone()));
    assert!(tips.set_hovered(HoverPlace::Overlay, tab0.clone(), true));
    assert_eq!(tips.active_target(), Some(&tab0));
    tips.sync_tabs(&["Alpha".into(), "Beta".into()]);
    assert_eq!(tips.active_target(), Some(&tab1));
}

#[gpui_kit::test]
fn sync_tabs_and_clear_prune_changed_labels_removed_tabs_and_reset_all() {
    let mut tips = RailTips::new();
    let tab0 = Target::new(RowKey::Tab(0), "Alpha");
    let tab1_old = Target::new(RowKey::Tab(1), "Beta");
    let tab1_new = Target::new(RowKey::Tab(1), "Gamma");
    let new_cmp = Target::new(RowKey::NewCompare, "New comparison");
    let rect = Bounds::new(point(px(4.), px(40.)), size(px(48.), px(30.)));

    tips.sync_focus(Some(tab0.clone()));
    assert!(tips.set_hovered(HoverPlace::Row, tab1_old.clone(), true));
    assert!(tips.set_bounds(RowKey::Tab(1), rect));
    assert!(tips.active_anchor().is_some());
    assert!(tips.dismiss());

    // Replacing label at index 1 prunes old row_hover/suppressed and invalidates anchor.
    tips.sync_tabs(&["Alpha".into(), "Gamma".into()]);
    assert_eq!(tips.active_target(), Some(&tab0));
    assert_eq!(tips.active_anchor(), None);
    assert!(tips.set_hovered(HoverPlace::Row, tab1_new.clone(), true));
    assert_eq!(tips.active_target(), Some(&tab1_new));

    // Removing index 1 drops row_hover, leaving kept Tab(0).
    tips.sync_tabs(&["Alpha".into()]);
    assert_eq!(tips.active_target(), Some(&tab0));

    // NewCompare survives empty tab list; `clear()` resets suppression and all targets.
    tips.sync_focus(Some(new_cmp.clone()));
    assert!(tips.set_bounds(RowKey::NewCompare, rect));
    assert!(tips.dismiss());
    tips.sync_tabs(&[]);
    assert_eq!(tips.active_target(), None);

    tips.clear();
    tips.sync_focus(Some(new_cmp.clone()));
    assert_eq!(tips.active_target(), Some(&new_cmp));
    assert_eq!(tips.active_anchor(), None);
}

#[gpui_kit::test]
fn bounds_and_active_anchor_require_matching_active_non_suppressed_positive_bounds() {
    let mut tips = RailTips::new();
    let new_cmp = Target::new(RowKey::NewCompare, "New comparison");
    let tab0 = Target::new(RowKey::Tab(0), "Alpha");
    let r1 = Bounds::new(point(px(4.), px(12.)), size(px(48.), px(30.)));
    let r2 = Bounds::new(point(px(4.), px(14.)), size(px(48.), px(30.)));
    let zero_w = Bounds::new(point(px(4.), px(12.)), size(px(0.), px(30.)));
    let neg_h = Bounds::new(point(px(4.), px(12.)), size(px(48.), px(-2.)));

    assert!(!tips.wants_bounds(RowKey::NewCompare));
    assert!(!tips.set_bounds(RowKey::NewCompare, r1));
    assert_eq!(tips.active_anchor(), None);

    tips.sync_focus(Some(new_cmp.clone()));
    assert!(tips.wants_bounds(RowKey::NewCompare));
    assert!(!tips.wants_bounds(RowKey::Tab(0)));
    assert!(!tips.set_bounds(RowKey::Tab(0), r1));
    assert!(!tips.set_bounds(RowKey::NewCompare, zero_w));
    assert!(!tips.set_bounds(RowKey::NewCompare, neg_h));
    assert_eq!(tips.active_anchor(), None);

    assert!(tips.set_bounds(RowKey::NewCompare, r1));
    assert!(!tips.set_bounds(RowKey::NewCompare, r1));
    assert_eq!(tips.active_anchor(), Some((&new_cmp, r1)));

    assert!(tips.set_bounds(RowKey::NewCompare, r2));
    assert_eq!(tips.active_anchor(), Some((&new_cmp, r2)));

    // Retargeting drops stale anchor even when returning to NewCompare later.
    assert!(tips.set_hovered(HoverPlace::Row, tab0, true));
    assert_eq!(tips.active_anchor(), None);
    assert!(tips.set_hovered(HoverPlace::Row, Target::new(RowKey::Tab(0), "Alpha"), false));
    assert_eq!(tips.active_anchor(), None);

    // Dismiss hides both `wants_bounds` and `active_anchor`.
    assert!(tips.set_bounds(RowKey::NewCompare, r2));
    assert!(tips.dismiss());
    assert!(!tips.wants_bounds(RowKey::NewCompare));
    assert_eq!(tips.active_anchor(), None);
}
