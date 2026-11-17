//! Tests of the Git side cache over fixture repositories.

use std::io::Read;
use std::sync::Arc;

use sidewise_core::{Location, ProviderPath};
use sidewise_engine::model::{CancelToken, EntryKind, Reason, RelPath, SideSource};
use sidewise_git::fixture::Fixture;
use sidewise_git::location::{Form, GitAt, encode};
use sidewise_host::compare::diff::hooks::SideFn;
use sidewise_host::compare::handle::CompareTarget;
use sidewise_host::compare::settings::CompareSettings;

use super::{GIT_SIDES, GitSides, side_fn, target_for};

fn at(fixture: &Fixture, rev: &str) -> GitAt {
    GitAt {
        repo: fixture.path().to_path_buf(),
        rev: rev.to_owned(),
        inner: String::new(),
        form: Form::Stored,
    }
}

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    let _ = fixture.commit(
        "main",
        &[("src/lib.rs", b"fn main() {}\n"), ("a.txt", b"a")],
    );
    fixture
}

/// A cache that opens with only the repository's own config (NC191).
fn isolated() -> Arc<GitSides> {
    Arc::new(GitSides::isolated())
}

/// The names in the root folder of `side`, sorted.
fn root_names(side: &dyn SideSource) -> Vec<String> {
    let mut names = Vec::new();
    side.list(
        &RelPath::new(ProviderPath::from_text("")),
        &mut |entry| {
            names.push(entry.rel.as_path().to_text());
            std::ops::ControlFlow::Continue(())
        },
        &CancelToken::new(),
    )
    .expect("list");
    names.sort();
    names
}

#[test]
fn same_key_shares_one_side() {
    let fixture = fixture();
    let sides = isolated();
    let first = sides.side(&at(&fixture, "main")).expect("open");
    let second = sides.side(&at(&fixture, "main")).expect("hit");
    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn dropped_side_is_released() {
    let fixture = fixture();
    let sides = isolated();
    let first = sides.side(&at(&fixture, "main")).expect("open");
    let weak = Arc::downgrade(&first);
    drop(first);
    assert!(
        weak.upgrade().is_none(),
        "the cache kept a closed side open"
    );
}

#[test]
fn live_side_survives_many_other_keys() {
    let fixture = fixture();
    let sides = isolated();
    let first = sides.side(&at(&fixture, "main")).expect("open");
    for n in 0..=GIT_SIDES {
        let branch = format!("b{n}");
        let _ = fixture.commit(&branch, &[("f", b"x")]);
        drop(sides.side(&at(&fixture, &branch)).expect("open"));
    }
    let again = sides.side(&at(&fixture, "main")).expect("hit");
    assert!(Arc::ptr_eq(&first, &again), "a live side was opened twice");
}

#[test]
fn past_the_bound_the_oldest_live_entry_goes() {
    let fixture = fixture();
    let sides = isolated();
    let first = sides.side(&at(&fixture, "main")).expect("open");
    let mut held = Vec::new();
    for n in 0..GIT_SIDES {
        let branch = format!("b{n}");
        let _ = fixture.commit(&branch, &[("f", b"x")]);
        held.push(sides.side(&at(&fixture, &branch)).expect("open"));
    }
    let again = sides.side(&at(&fixture, "main")).expect("reopen");
    assert!(!Arc::ptr_eq(&first, &again));
}

#[test]
fn new_run_sees_a_moved_branch_and_old_side_keeps_its_commit() {
    let fixture = fixture();
    let sides = isolated();
    let target = target_for(Arc::clone(&sides));
    let root = encode(fixture.path(), "main", Form::Stored).expect("encode");
    let CompareTarget::Side(open) = target(&root) else {
        panic!("a Git root is an opener");
    };
    let old = open().expect("open");
    let _ = fixture.commit("main", &[("a.txt", b"a"), ("b.txt", b"b")]);
    let CompareTarget::Side(open) = target(&root) else {
        panic!("a Git root is an opener");
    };
    let new = open().expect("reopen");
    assert_eq!(root_names(new.as_ref()), ["a.txt", "b.txt"]);
    assert_eq!(root_names(old.as_ref()), ["a.txt", "src"]);
    let bytes = file_at(&side_fn(sides), &fixture, "main", "b.txt").expect("newest");
    assert_eq!(bytes, "b");
}

#[test]
fn side_fn_refuses_a_folder() {
    let fixture = fixture();
    let side = side_fn(isolated());
    let root = encode(fixture.path(), "main", Form::Stored).expect("encode");
    let text = format!("{}/src", root.path().to_text());
    let location = Location::with_provider(root.provider().clone(), ProviderPath::from_text(&text))
        .expect("location");
    let found = side(&location, &CompareSettings::default()).map(|(_, entry)| entry.kind);
    assert_eq!(found.err(), Some(Reason::Unsupported));
}

#[test]
fn local_location_is_local_target() {
    let target = target_for(isolated());
    let location = Location::local(ProviderPath::from_text("/tmp")).expect("local");
    assert!(matches!(target(&location), CompareTarget::Local(_)));
}

#[test]
fn git_root_opens_and_lists() {
    let fixture = fixture();
    let target = target_for(isolated());
    let root = encode(fixture.path(), "main", Form::Stored).expect("encode");
    let CompareTarget::Side(open) = target(&root) else {
        panic!("a Git root is an opener");
    };
    let side = open().expect("open");
    assert_eq!(root_names(side.as_ref()), ["a.txt", "src"]);
}

fn file_at(side: &SideFn, fixture: &Fixture, rev: &str, inner: &str) -> Result<String, Reason> {
    let root = encode(fixture.path(), rev, Form::Stored).expect("encode");
    let text = format!("{}/{inner}", root.path().to_text());
    let location = Location::with_provider(root.provider().clone(), ProviderPath::from_text(&text))
        .expect("location");
    let (source, entry) = side(&location, &CompareSettings::default())?;
    let mut bytes = String::new();
    source
        .open(&entry.rel, sidewise_engine::model::ReadHint::Buffered)?
        .read_to_string(&mut bytes)
        .expect("read");
    assert_eq!(entry.kind, EntryKind::File);
    Ok(bytes)
}

#[test]
fn side_fn_reads_a_git_file() {
    let fixture = fixture();
    let side = side_fn(isolated());
    let bytes = file_at(&side, &fixture, "main", "src/lib.rs").expect("read");
    assert_eq!(bytes, "fn main() {}\n");
}

#[test]
fn side_fn_bad_rev_is_not_found() {
    let fixture = fixture();
    let side = side_fn(isolated());
    assert_eq!(
        file_at(&side, &fixture, "nope", "src/lib.rs").err(),
        Some(Reason::NotFound)
    );
}
