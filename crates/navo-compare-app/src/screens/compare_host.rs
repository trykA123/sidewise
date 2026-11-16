//! The compare host: a compare screen over the default diff backend and the compare keys.
//! The compare tab mounts these (H2).

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{App, KeyBinding, KeyBindingContextPredicate};
use sidewise_host::compare::diff::hooks::DiffBackend;
use sidewise_ui::browser::Waker;
use sidewise_ui::compare_diff::{CompareOptions, DiffHooks};
use sidewise_ui::compare_keys::compare_keys;
use sidewise_ui::window_view::{CompareScreen, DiffSlot};

use super::host_parts::compare_files;

#[cfg(test)]
thread_local! {
    /// Test seam: diff hooks that replace the in-memory backend on this thread.
    pub(crate) static TEST_DIFF_HOOKS: std::cell::RefCell<Option<Arc<dyn DiffHooks>>> =
        const { std::cell::RefCell::new(None) };
}

/// The diff hooks a new screen uses: the in-memory backend, or the test seam's hooks.
fn diff_hooks() -> Arc<dyn DiffHooks> {
    #[cfg(test)]
    if let Some(hooks) = TEST_DIFF_HOOKS.with_borrow(Clone::clone) {
        return hooks;
    }
    Arc::new(DiffBackend::default())
}

/// A compare screen over the in-memory diff backend (NC118); `wake` runs after each diff update.
pub fn new_compare_screen(wake: Waker) -> CompareScreen {
    let hooks = diff_hooks();
    CompareScreen::new(
        DiffSlot::new(Some(hooks), wake),
        CompareOptions::default(),
        compare_files(),
    )
}

/// Binds Navo's compare keys (contexts `CompareView`, `CompareStream`); returns how many bindings
/// it added. A bind whose action or keystroke does not load is skipped.
pub fn bind_compare_keys(cx: &mut App) -> usize {
    let Ok(keys) = compare_keys() else {
        return 0;
    };
    let mut all = Vec::new();
    for entry in keys {
        let Some(name) = action_name(&entry.command) else {
            continue;
        };
        let Ok(action) = cx.build_action(&name, None) else {
            continue;
        };
        let predicate = KeyBindingContextPredicate::parse(&entry.context)
            .ok()
            .map(Rc::new);
        for key in &entry.keys {
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

/// The action type name of a compare command (`compare.next_difference` becomes
/// `compare::NextDifference`); `None` when the command has no namespace dot.
pub(crate) fn action_name(command: &str) -> Option<String> {
    let (namespace, rest) = command.split_once('.')?;
    let action = rest
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            let first = chars.next()?;
            Some(first.to_uppercase().collect::<String>() + chars.as_str())
        })
        .collect::<Option<Vec<_>>>()?
        .join("");
    Some(format!("{namespace}::{action}"))
}
