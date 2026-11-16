//! The compare tab: owns one `CompareRun`, drains its updates on the UI thread and shows the
//! Folders view over the rows. The shell mounts it in place of the Start tab (H2).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use gpui_kit::{AsyncApp, FocusHandle, Focusable, Global, WeakEntity};
use navo::compare::PaneCompare;
use navo_ui::browser::Waker;
use navo_ui::prelude::*;

use super::compare_run::{CompareRun, RunError};
use super::folders_view::FoldersView;
use crate::start_tabs::StartChoice;

/// Pause between two wake updates, about one frame.
const PAUSE: Duration = Duration::from_millis(16);

/// Key context of the tab root; the tab keys bind inside it later.
const CONTEXT: &str = "CompareTab";

/// The two sides and the start choice a compare tab opens with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompareRequest {
    pub left: PathBuf,
    pub right: PathBuf,
    pub choice: StartChoice,
}

/// Which screen the tab shows; only `Folders` is used until the file, stream and large screens land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(dead_code, reason = "H3 to H5 show the File, Stream and Large screens")]
pub enum TabView {
    Large,
    Folders,
    File,
    Stream,
}

/// The app's one folder-compare engine, shared by every compare tab.
struct CompareHooks(Arc<PaneCompare>);

impl Global for CompareHooks {}

/// Sets the app's one `Arc<PaneCompare>` global; `window::run` calls it once, tests call it before opening a shell.
pub fn install_compare_hooks(cx: &mut App) {
    cx.set_global(CompareHooks(Arc::new(PaneCompare::default())));
}

/// A started run and the receiver of its wake signals, not yet tied to an entity.
pub struct Started {
    run: CompareRun,
    wakes: UnboundedReceiver<()>,
}

/// One compare tab: the run, the shown view, the Folders view, its title and focus handle.
pub struct CompareTab {
    run: CompareRun,
    view: TabView,
    folders: Entity<FoldersView>,
    title: SharedString,
    focus: FocusHandle,
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
            wake,
        )?;
        Ok(Started { run, wakes })
    }

    /// Makes the tab over a started run and arms the wake loop.
    pub fn from_started(
        started: Started,
        request: &CompareRequest,
        cx: &mut Context<Self>,
    ) -> Self {
        let Started { run, wakes } = started;
        let folders = cx.new(FoldersView::new);
        cx.spawn(async move |this, cx| pump(wakes, this, cx).await)
            .detach();
        let mut tab = Self {
            run,
            view: TabView::Folders,
            folders,
            title: title_of(request),
            focus: cx.focus_handle(),
        };
        tab.on_wake(cx);
        tab
    }

    /// Starts the run and builds the tab; for a caller that already holds the entity's context.
    #[expect(
        dead_code,
        reason = "the shell opens tabs through `start` and `from_started`"
    )]
    pub fn new(
        request: CompareRequest,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, RunError> {
        let started = Self::start(&request, cx)?;
        Ok(Self::from_started(started, &request, cx))
    }

    /// The tab's title: both folder names around an arrow.
    pub fn title(&self) -> SharedString {
        self.title.clone()
    }

    /// The Folders view entity (lets tests and the shell read its state).
    pub fn folders(&self) -> &Entity<FoldersView> {
        &self.folders
    }

    /// Focuses the shown view's keys.
    pub fn focus_view(&self, window: &mut Window, cx: &mut Context<Self>) {
        match self.view {
            TabView::Folders => self.folders.read(cx).focus_handle(cx).focus(window, cx),
            _ => self.focus.focus(window, cx),
        }
    }

    /// Applies queued updates; when any changed the session, hands the rows to the Folders view.
    fn on_wake(&mut self, cx: &mut Context<Self>) {
        if !self.run.drain() {
            return;
        }
        if let Some(session) = self.run.session() {
            let rows = session.rows_under_folder();
            self.folders.update(cx, |view, cx| view.set_rows(rows, cx));
        }
        cx.notify();
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
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .child(self.folders.clone())
    }
}
