//! The app's one cache of open Git sides, shared by the folder session and the diff builder.
//! A screen of the compare tab; the opener runs on the session thread, never the UI thread.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use sidewise_core::{Location, ProviderPath};
use sidewise_engine::model::{EntryKind, Reason, RelPath, SideEntry, SideSource};
use sidewise_git::location::{Form, GitAt, parse};
use sidewise_git::side::GitTreeSide;
use sidewise_host::compare::diff::hooks::SideFn;
use sidewise_host::compare::diff::sides::side_of;
use sidewise_host::compare::handle::CompareTarget;
use sidewise_host::compare::pane::LocalTarget;

/// How many open sides the cache keeps.
pub const GIT_SIDES: usize = 8;

/// The app's one cache, shared by the compare engine and every diff backend; made on first use.
pub fn shared_sides() -> Arc<GitSides> {
    static SIDES: OnceLock<Arc<GitSides>> = OnceLock::new();
    Arc::clone(SIDES.get_or_init(Arc::default))
}

/// What names a side: its form, repository folder and revision text.
type Key = (Form, PathBuf, String);

/// An LRU cache (least recently used dropped first) of open Git sides, most recent last.
#[derive(Default)]
pub struct GitSides(Mutex<Vec<(Key, Arc<GitTreeSide>)>>);

impl GitSides {
    /// The open side for `at`, opened on a miss. Fails as `GitTreeSide::open`. Blocking on a
    /// miss: call off the UI thread. The open runs outside the lock.
    pub fn side(&self, at: &GitAt) -> Result<Arc<GitTreeSide>, Reason> {
        let key: Key = (at.form, at.repo.clone(), at.rev.clone());
        if let Some(hit) = self.touch(&key) {
            return Ok(hit);
        }
        let opened = Arc::new(GitTreeSide::open(&at.repo, &at.rev, at.form)?);
        let mut list = lock(&self.0);
        // Another thread may have opened the same key meanwhile: keep its side.
        if let Some(found) = list.iter().position(|(held, _)| *held == key) {
            let entry = list.remove(found);
            let side = Arc::clone(&entry.1);
            list.push(entry);
            return Ok(side);
        }
        list.push((key, Arc::clone(&opened)));
        if list.len() > GIT_SIDES {
            list.remove(0);
        }
        Ok(opened)
    }

    /// The side held under `key`, moved to the most recent place.
    fn touch(&self, key: &Key) -> Option<Arc<GitTreeSide>> {
        let mut list = lock(&self.0);
        let found = list.iter().position(|(held, _)| held == key)?;
        let entry = list.remove(found);
        let side = Arc::clone(&entry.1);
        list.push(entry);
        Some(side)
    }
}

/// Locks the cache, taking the list back from a poisoned lock: it is only ever whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A target builder: local locations open as local folders, Git locations through `sides`
/// (an unparsable Git location opens as `Reason::Unsupported`).
pub fn target_for(sides: Arc<GitSides>) -> LocalTarget {
    Arc::new(move |location: &Location| {
        if location.is_local() {
            return CompareTarget::Local(location.clone());
        }
        let parsed = parse(location);
        let sides = Arc::clone(&sides);
        CompareTarget::Side(Box::new(move || {
            let at = parsed.map_err(|_| Reason::Unsupported)?;
            let side: Arc<dyn SideSource> = sides.side(&at)?;
            Ok(side)
        }))
    })
}

/// A diff side builder: local files as `side_of`, Git files from `sides`. A missing path
/// or revision is `NotFound`; any other provider is `Unsupported`.
pub fn side_fn(sides: Arc<GitSides>) -> SideFn {
    Arc::new(move |location, settings| {
        if location.is_local() {
            return side_of(location, settings);
        }
        let at = parse(location).map_err(|_| Reason::Unsupported)?;
        let side = sides.side(&at)?;
        let rel = RelPath::new(ProviderPath::from_text(&at.inner));
        let stamp = side.stamp(&rel)?;
        let entry = SideEntry::new(rel, EntryKind::File, stamp.size, None);
        let source: Arc<dyn SideSource> = side;
        Ok((source, entry))
    })
}

#[cfg(test)]
#[path = "git_sides_tests.rs"]
mod tests;
