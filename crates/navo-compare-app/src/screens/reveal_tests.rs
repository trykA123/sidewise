//! Tests of the Reveal argument builder; they run on every OS.

use std::path::{Path, PathBuf};

use super::select_arg;

/// A temp directory holding a file named `name`; returns its path.
fn temp_file(tag: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sw-reveal-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let file = dir.join(name);
    std::fs::write(&file, b"x").expect("file");
    file
}

#[test]
fn select_arg_quotes_one_path() {
    let file = temp_file("ok", "a, b.txt");
    let text = file.to_str().expect("utf-8");
    assert_eq!(select_arg(&file), Ok(format!("/select,\"{text}\"")));
}

#[test]
fn select_arg_refuses_a_relative_path() {
    assert!(select_arg(Path::new("a/b.txt")).is_err());
}

#[test]
fn select_arg_refuses_a_missing_path() {
    let file = temp_file("gone", "x.txt");
    let missing = file.with_file_name("nope.txt");
    assert!(select_arg(&missing).is_err());
}

#[cfg(unix)]
#[test]
fn select_arg_refuses_a_quote() {
    let file = temp_file("quote", "a\"b.txt");
    assert!(select_arg(&file).is_err());
}

#[cfg(unix)]
#[test]
fn select_arg_refuses_a_control_character() {
    let file = temp_file("ctl", "a\u{1f}b.txt");
    assert!(select_arg(&file).is_err());
}

#[cfg(not(windows))]
#[test]
fn reveal_off_windows_gives_the_notice() {
    let file = temp_file("off", "x.txt");
    assert_eq!(super::reveal(&file), Err(super::OFF_WINDOWS.to_owned()));
}
