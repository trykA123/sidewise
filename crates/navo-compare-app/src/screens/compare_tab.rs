//! The compare tab: owns one `CompareRun`, drains its updates on the UI thread and shows the
//! Folders view over the rows. The shell mounts it in place of the Start tab (H2).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use gpui_kit::{AsyncApp, FocusHandle, Focusable, Global, Subscription, WeakEntity};
use navo::compare::PaneCompare;
use navo_ui::browser::Waker;
use navo_ui::prelude::*;
use navo_ui::window_view::CompareScreen;

use super::compare_host::new_compare_screen;
use super::compare_run::{CompareRun, RunError};
use super::compare_tab_large::large_timer;
use super::folders_view::FoldersView;
use super::large_view::LargeView;
use crate::start_tabs::StartChoice;

/// Pause between two wake updates, about one frame.
const PAUSE: Duration = Duration::from_millis(16);

/// Key context of the tab root; the tab keys bind inside it.
pub(super) const CONTEXT: &str = "CompareTab";

/// Extra context token while the tab's own views (Folders, Large) show; the digit keys bind to it,
/// so digits typed inside the compare screen reach Navo.
pub(crate) const OWN: &str = "CompareTabOwn";

/// The tab root's context string while `OWN` is set; keep it equal to `CONTEXT` plus `OWN`.
const OWN_CONTEXTS: &str = "CompareTab CompareTabOwn";

/// The two sides and the start choice a compare tab opens with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompareRequest {
    pub left: PathBuf,
    pub right: PathBuf,
    pub choice: StartChoice,
}

/// Which screen the tab shows; `File` and `Stream` both use the one compare screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabView {
    Large,
    Folders,
    File,
    Stream,
}

/// The app's one folder-compare engine, shared by every compare tab.
struct CompareHooks(Arc<dyn navo_ui::browser::CompareHooks>);

impl Global for CompareHooks {}

/// Sets the app's one `Arc<PaneCompare>` global; `window::run` calls it once, tests call it before opening a shell.
pub fn install_compare_hooks(cx: &mut App) {
    cx.set_global(CompareHooks(Arc::new(PaneCompare::default())));
}

/// Test hook: replaces the compare engine with a scripted one.
#[cfg(test)]
pub(crate) fn install_test_hooks(hooks: Arc<dyn navo_ui::browser::CompareHooks>, cx: &mut App) {
    cx.set_global(CompareHooks(hooks));
}

/// A started run and the receiver of its wake signals, not yet tied to an entity.
pub struct Started {
    run: CompareRun,
    wakes: UnboundedReceiver<()>,
    wake: Waker,
}

/// One compare tab: the run, the shown view, the Folders view, its title and focus handle.
pub struct CompareTab {
    pub(super) run: CompareRun,
    pub(super) view: TabView,
    pub(super) folders: Entity<FoldersView>,
    pub(super) large: Entity<LargeView>,
    /// True while Large shows because the 1,000 ms timer put it there; the session's end then ends it.
    pub(super) auto_large: bool,
    /// Set when a view change needs the window to move focus; `render` does it.
    pub(super) focus_pending: bool,
    pub(super) screen: Entity<CompareScreen>,
    pub(super) shown: Option<u64>,
    title: SharedString,
    focus: FocusHandle,
    pub(super) subscriptions: Vec<Subscription>,
}

impl CompareTab {
    /// Starts the run for `request`; a bad side returns `RunError` before any entity exists.
    /// The shell calls this outside `cx.new`, so an error leaves no half-made tab behind.
    pub fn start(request: &CompareRequest, cx: &App) -> Result<Started, RunError> {
        let hooks = cx
            .try_global::<CompareHooks>()
            .map(|hooks| Arc::clone(&hooks.0))
            .ok_or_else(|| RunError::Location("the compare engine is not installed".to_owned()))?;
        let (sender, wakes) = unbounded();
        let wake: Waker = Arc::new(move || {
            let _ = sender.unbounded_send(());
        });
        let run = CompareRun::start(
            (&request.left, &request.right),
            &request.choice,
            &*hooks,
            Arc::clone(&wake),
        )?;
        Ok(Started { run, wakes, wake })
    }

    /// Makes the tab over a started run and arms the wake loop.
    pub fn from_started(
        started: Started,
        request: &CompareRequest,
        cx: &mut Context<Self>,
    ) -> Self {
        let Started { run, wakes, wake } = started;
        let roots = [request.left.clone(), request.right.clone()];
        let folders = cx.new(|cx| FoldersView::new(cx).with_roots(roots));
        let large = cx.new(LargeView::new);
        let screen = cx.new(|_| new_compare_screen(wake));
        cx.spawn(async move |this, cx| pump(wakes, this, cx).await)
            .detach();
        cx.spawn(async move |this, cx| large_timer(this, cx).await)
            .detach();
        let mut tab = Self {
            run,
            view: TabView::Folders,
            folders,
            large,
            auto_large: false,
            focus_pending: false,
            screen,
            shown: None,
            title: title_of(request),
            focus: cx.focus_handle(),
            subscriptions: Vec::new(),
        };
        tab.on_wake(cx);
        tab
    }

    /// Starts the run and builds the tab; for a caller that already holds the entity's context.
    #[expect(
        dead_code,
        reason = "the shell opens tabs through `start`, `from_started` and `attach`"
    )]
    pub fn new(
        request: CompareRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, RunError> {
        let started = Self::start(&request, cx)?;
        let mut tab = Self::from_started(started, &request, cx);
        tab.attach(&request, window, cx);
        Ok(tab)
    }

    /// The tab's title: both folder names around an arrow.
    pub fn title(&self) -> SharedString {
        self.title.clone()
    }

    /// The Folders view entity (lets tests and the shell read its state).
    pub fn folders(&self) -> &Entity<FoldersView> {
        &self.folders
    }

    /// The Large view entity; for tests.
    #[cfg(test)]
    pub(crate) fn large(&self) -> &Entity<LargeView> {
        &self.large
    }

    /// The compare screen entity that shows the file diff.
    pub fn screen(&self) -> &Entity<CompareScreen> {
        &self.screen
    }

    /// The shown view.
    pub fn view(&self) -> TabView {
        self.view
    }

    /// The id of the pair last opened in the file diff.
    pub fn shown_pair(&self) -> Option<u64> {
        self.shown
    }

    /// The id of the session row at `path`; for tests.
    #[cfg(test)]
    pub(crate) fn row_id(&self, path: &str) -> Option<u64> {
        let rows = self.run.session()?.rows_under_folder();
        rows.iter().find(|row| row.path == path).map(|row| row.id)
    }

    /// Focuses the shown view's keys.
    pub fn focus_view(&self, window: &mut Window, cx: &mut Context<Self>) {
        match self.view {
            TabView::Folders => self.folders.read(cx).focus_handle(cx).focus(window, cx),
            TabView::File => {
                let focused = self
                    .screen
                    .update(cx, |screen, cx| screen.focus_file_screen(window, cx));
                if !focused {
                    self.focus.focus(window, cx);
                }
            }
            TabView::Large => self.large.read(cx).focus_handle(cx).focus(window, cx),
            TabView::Stream => self.focus.focus(window, cx),
        }
    }

    /// Applies queued updates: new session rows go to the Folders view, then the compare screen
    /// polls its diff inboxes. Notifies when either changed.
    fn on_wake(&mut self, cx: &mut Context<Self>) {
        let drained = self.run.drain();
        if drained && let Some(session) = self.run.session() {
            let rows = session.rows_under_folder();
            self.folders.update(cx, |view, cx| view.set_rows(rows, cx));
            self.sync_large(cx);
        }
        let polled = self
            .screen
            .update(cx, |screen, cx| screen.poll_compare_screen(false, cx));
        if drained || polled {
            cx.notify();
        }
    }
}

/// Applies wakes to the tab until the wake channel or the tab closes. Queued wakes merge into
/// one update, and a 16 ms pause after it lets the UI paint while a large walk keeps sending.
async fn pump(mut wakes: UnboundedReceiver<()>, this: WeakEntity<CompareTab>, cx: &mut AsyncApp) {
    while wakes.next().await.is_some() {
        while wakes.try_recv().is_ok() {}
        // A failed upgrade means the tab closed; the walk's thread may still wake.
        if this.update(cx, |tab, cx| tab.on_wake(cx)).is_err() {
            return;
        }
        cx.background_executor().timer(PAUSE).await;
    }
}

/// `"<left name> ↔ <right name>"` from the last path segment of each side.
fn title_of(request: &CompareRequest) -> SharedString {
    let name = |path: &PathBuf| {
        path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    };
    format!("{} ↔ {}", name(&request.left), name(&request.right)).into()
}

impl Render for CompareTab {
    /// The shown view inside the tab's key context.
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_pending) {
            self.focus_view(window, cx);
        }
        let shown = match self.view {
            TabView::Folders => self.folders.clone().into_any_element(),
            TabView::Large => self.large.clone().into_any_element(),
            TabView::File | TabView::Stream => self.screen.clone().into_any_element(),
        };
        let own = matches!(self.view, TabView::Folders | TabView::Large);
        let root = div()
            .key_context(if own { OWN_CONTEXTS } else { CONTEXT })
            .track_focus(&self.focus)
            // Navo's diff and Stream roots are `flex_1`: they need a flex parent to get a height.
            .flex()
            .flex_col()
            .size_full()
            .child(shown);
        self.with_keys(root, cx)
    }
}
