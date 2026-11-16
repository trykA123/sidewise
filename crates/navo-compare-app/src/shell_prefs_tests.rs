//! Tests of the View preferences global: reading the profile, saving back and the defaults.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use gpui_kit::TestAppContext;
use navo_compare_profile::appearance::UiPrefs;
use navo_compare_profile::brand::SLUG;
use navo_compare_profile::paths::open_profile;

use crate::shell_prefs::{install_prefs, prefs, save_prefs};

static COUNT: AtomicUsize = AtomicUsize::new(0);

/// A temporary folder for one profile, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    /// Makes an empty folder named after the test and a counter.
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "{SLUG}-vm2-{}-{name}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&path).expect("create temp folder");
        Self(path)
    }

    /// The folder path.
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    /// Removes the folder and everything in it.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The settings of the profile on disk are read into the installed global.
#[gpui_kit::test]
fn prefs_read_the_profile_settings(cx: &mut TestAppContext) {
    let temp = TempDir::new("read");
    let profile = open_profile(Some(temp.path())).expect("open profile");
    fs::write(profile.settings_file(), "[ui]\nwords = false\n").expect("write settings");
    cx.update(|cx| install_prefs(Some(temp.path()), cx))
        .expect("install");
    assert!(!cx.update(|cx| prefs(cx)).words, "words = false comes back");
}

/// Saved preferences are written to the profile and survive a reinstall.
#[gpui_kit::test]
fn saved_prefs_survive_a_reinstall(cx: &mut TestAppContext) {
    let temp = TempDir::new("save");
    cx.update(|cx| install_prefs(Some(temp.path()), cx))
        .expect("install");
    let wanted = UiPrefs {
        words: false,
        wrap_lines: true,
        ..UiPrefs::default()
    };
    cx.update(|cx| save_prefs(wanted.clone(), cx))
        .expect("save");
    cx.update(|cx| install_prefs(Some(temp.path()), cx))
        .expect("reinstall");
    assert_eq!(
        cx.update(|cx| prefs(cx)),
        wanted,
        "the saved preferences come back"
    );
}

/// Without an installed global the preferences are the defaults.
#[gpui_kit::test]
fn missing_global_gives_the_defaults(cx: &mut TestAppContext) {
    assert_eq!(cx.update(|cx| prefs(cx)), UiPrefs::default());
}
