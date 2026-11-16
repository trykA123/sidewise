//! Tests for the compare host: key binding count, read-only file hooks and opening a text pair.

use std::fs;
use std::sync::Arc;

use navo_ui::browser::Waker;
use navo_ui::commands::{COMPARE_KEYMAP, Keymap};
use navo_ui::compare_diff::PairRequest;

use super::compare_host::{bind_compare_keys, new_compare_screen, read_only_file_hooks};
use super::compare_run::location_of;

/// The binding count equals the keys of every parsed bind that has an action.
#[gpui_kit::test]
fn bind_compare_keys_binds_every_compare_key(cx: &mut gpui_kit::TestAppContext) {
    let keymap = Keymap::parse(COMPARE_KEYMAP).expect("parse compare keymap");
    let expected: usize = keymap
        .binds()
        .iter()
        .filter(|bind| bind.spec.action.is_some())
        .map(|bind| bind.keys.len())
        .sum();
    let added = cx.update(|cx| {
        gpui_kit::init(cx);
        bind_compare_keys(cx)
    });
    assert!(expected > 0);
    assert_eq!(added, expected);
}

/// The hooks read a local file as text and as bytes.
#[test]
fn read_only_file_hooks_read_local_files() {
    let path = std::env::temp_dir().join(format!("sidewise-host-{}.txt", std::process::id()));
    fs::write(&path, b"hello").expect("write temp file");
    let loc = location_of(&path).expect("location");
    let hooks = read_only_file_hooks();
    let text = (hooks.read)(&loc).expect("read text");
    let bytes = (hooks.bytes)(&loc, 64).expect("read bytes");
    let _ = fs::remove_file(&path);
    assert_eq!(text.text, "hello");
    assert_eq!(bytes, b"hello");
}

/// A text pair opens in the file diff.
#[gpui_kit::test]
fn compare_screen_opens_a_text_pair(cx: &mut gpui_kit::TestAppContext) {
    let dir = std::env::temp_dir().join(format!("sidewise-host-pair-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create dir");
    fs::write(dir.join("a.txt"), b"one\n").expect("write left");
    fs::write(dir.join("b.txt"), b"two\n").expect("write right");
    let wake: Waker = Arc::new(|| {});
    let (window, screen) = crate::test_window::open(cx, |_, _| new_compare_screen(wake));
    let request = PairRequest {
        id: 0,
        path: "a.txt".into(),
        left: Some(location_of(&dir.join("a.txt")).expect("left")),
        right: Some(location_of(&dir.join("b.txt")).expect("right")),
    };
    let opened = cx.update(|cx| {
        window
            .update(cx, |_, window, cx| {
                screen.update(cx, |screen, cx| {
                    screen.show_pair((request, None), window, cx)
                })
            })
            .expect("update window")
    });
    let open = screen.read_with(cx, |screen, _| screen.is_file_open());
    let _ = fs::remove_dir_all(&dir);
    assert!(opened);
    assert!(open);
}
