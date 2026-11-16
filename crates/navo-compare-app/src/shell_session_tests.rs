//! View tests of the recent-session wiring: compares are recorded, rows follow the store, a row reopens.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use navo_compare_cli::args::SideSpec;
use navo_compare_profile::book::{SessionBook, Visit};
use navo_compare_profile::brand::SLUG;
use navo_compare_profile::session_store::SessionStore;

use crate::screens::compare_tab::install_compare_hooks;
use crate::shell::Shell;
use crate::start::StartEvent;
use crate::start_tabs::StartChoice;
use crate::test_window::open;

static COUNT: AtomicUsize = AtomicUsize::new(0);

/// An exclusive temp folder holding `left/` and `right/`, removed on drop.
struct Fixture(PathBuf);

impl Fixture {
    /// Makes the folder and its two sides.
    fn new() -> Self {
        let name = format!(
            "{SLUG}-h6b-{}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::SeqCst)
        );
        let path = std::env::temp_dir().join(name);
        fs::create_dir(&path).expect("create fixture");
        fs::create_dir(path.join("left")).expect("left");
        fs::create_dir(path.join("right")).expect("right");
        Self(path)
    }

    /// The left side.
    fn left(&self) -> PathBuf {
        self.0.join("left")
    }

    /// The right side.
    fn right(&self) -> PathBuf {
        self.0.join("right")
    }

    /// The profile folder the book makes.
    fn profile(&self) -> PathBuf {
        self.0.join("profile")
    }

    /// The sessions the store holds.
    fn stored(&self) -> Vec<navo_compare_profile::session::Session> {
        SessionStore::open(self.profile().join("sessions"))
            .list()
            .expect("list")
    }
}

impl Drop for Fixture {
    /// Removes the folder.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Opens a shell, with the session book over `profile` when given, and renders one frame.
fn open_shell(cx: &mut TestAppContext, profile: Option<&Path>) -> (AnyWindowHandle, Entity<Shell>) {
    cx.update(|cx| {
        install_compare_hooks(cx);
        if let Some(dir) = profile {
            Shell::install_sessions(Some(dir), cx).expect("book");
        }
    });
    let opened = open(cx, |window, cx| {
        let shell = Shell::new(window, cx);
        shell.sync_rows(cx);
        shell
    });
    cx.update_window(opened.0, |_, window, cx| window.render_frame(cx))
        .expect("window");
    opened
}

/// Makes the active start screen emit `event`, then renders and settles.
fn emit(
    cx: &mut TestAppContext,
    (window, shell): &(AnyWindowHandle, Entity<Shell>),
    event: StartEvent,
) {
    cx.update_window(*window, |_, window, cx| {
        let screen = shell.read(cx).active_screen().cloned().expect("start tab");
        screen.update(cx, |_, cx| cx.emit(event));
        window.render_frame(cx);
    })
    .expect("window");
    cx.run_until_parked();
}

/// A compare event over the fixture's two sides.
fn compare(fixture: &Fixture) -> StartEvent {
    StartEvent::Compare {
        left: SideSpec::Path(fixture.left()),
        right: SideSpec::Path(fixture.right()),
        choice: StartChoice::default(),
    }
}

#[gpui_kit::test]
fn a_compare_from_start_is_saved_and_listed(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let opened = open_shell(cx, Some(&fixture.profile()));
    emit(cx, &opened, compare(&fixture));
    assert_eq!(fixture.stored().len(), 1);
    let shell = &opened.1;
    let sidebar = shell.read_with(cx, |shell, _| shell.sidebar().clone());
    assert_eq!(
        sidebar.read_with(cx, |sidebar, _| sidebar.rows().recent.len()),
        1
    );
    cx.update_window(opened.0, |_, window, cx| {
        shell.update(cx, |shell, cx| shell.new_tab(window, cx));
    })
    .expect("window");
    let screen = shell.read_with(cx, |shell, _| {
        shell.active_screen().cloned().expect("start")
    });
    assert_eq!(
        screen.read_with(cx, |screen, _| screen.rows.recent.len()),
        1
    );
}

#[gpui_kit::test]
fn an_open_row_reopens_and_bumps_its_session(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let mut book = SessionBook::open(Some(&fixture.profile()), 0).expect("book");
    let visit = Visit {
        left: fixture.left(),
        right: fixture.right(),
        respect_gitignore: false,
    };
    book.record(&visit, 1_000).expect("record");
    let id = book.rows().recent[0].id.clone();
    let opened = open_shell(cx, Some(&fixture.profile()));
    emit(cx, &opened, StartEvent::Open { id });
    let shell = &opened.1;
    assert!(shell.read_with(cx, |shell, _| shell.active_screen().is_none()));
    assert_eq!(shell.read_with(cx, |shell, _| shell.tab_count()), 1);
    let stored = fixture.stored();
    assert_eq!(stored.len(), 1);
    assert!(stored[0].opened > 1);
}

#[gpui_kit::test]
fn a_missing_session_keeps_start_and_shows_one_toast(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let opened = open_shell(cx, Some(&fixture.profile()));
    emit(cx, &opened, StartEvent::Open { id: "abc-0".into() });
    let shell = &opened.1;
    assert!(shell.read_with(cx, |shell, _| shell.active_screen().is_some()));
    assert_eq!(shell.read_with(cx, |shell, cx| shell.toast_count(cx)), 1);
}

#[gpui_kit::test]
fn without_a_book_a_compare_still_opens(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let opened = open_shell(cx, None);
    emit(cx, &opened, compare(&fixture));
    let shell = &opened.1;
    assert!(shell.read_with(cx, |shell, _| shell.active_screen().is_none()));
    assert_eq!(shell.read_with(cx, |shell, cx| shell.toast_count(cx)), 0);
}
