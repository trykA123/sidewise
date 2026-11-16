//! The compare host: read-only file hooks, a compare screen over the in-memory diff backend,
//! and the Navo compare keys. The compare tab mounts these (H2).

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{App, KeyBinding, KeyBindingContextPredicate};
use navo::compare::diff::DiffBackend;
use navo_ui::browser::Waker;
use navo_ui::commands::{COMPARE_KEYMAP, Keymap};
use navo_ui::compare_diff::{CompareOptions, DiffHooks};
use navo_ui::window_view::{CompareScreen, DiffSlot, FileHooks};

/// Navo's file readers for the compare screen (hex reads `paged`, pictures read `bytes`), with
/// every save refused.
pub fn read_only_file_hooks() -> FileHooks {
    FileHooks {
        write: FileHooks::none().write,
        ..navo::compare::file_hooks(None)
    }
}

/// A compare screen over the in-memory diff backend (NC118); `wake` runs after each diff update.
pub fn new_compare_screen(wake: Waker) -> CompareScreen {
    let hooks: Arc<dyn DiffHooks> = Arc::new(DiffBackend::in_memory());
    CompareScreen::new(
        DiffSlot::new(Some(hooks), wake),
        CompareOptions::default(),
        read_only_file_hooks(),
    )
}

/// Binds Navo's compare keys (contexts `CompareView`, `CompareStream`); returns how many bindings
/// it added. A bind whose action or keystroke does not load is skipped.
pub fn bind_compare_keys(cx: &mut App) -> usize {
    let Ok(keymap) = Keymap::parse(COMPARE_KEYMAP) else {
        return 0;
    };
    let mut all = Vec::new();
    for bind in keymap.binds() {
        let Some(name) = bind.spec.action else {
            continue;
        };
        let Ok(action) = cx.build_action(name, None) else {
            continue;
        };
        let predicate = KeyBindingContextPredicate::parse(&bind.context)
            .ok()
            .map(Rc::new);
        for key in &bind.keys {
            let action = action.boxed_clone();
            if let Ok(binding) = KeyBinding::load(key, action, predicate.clone(), false, None, cx) {
                all.push(binding);
            }
        }
    }
    let count = all.len();
    cx.bind_keys(all);
    count
}
