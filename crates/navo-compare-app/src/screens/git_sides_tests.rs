//! Tests of the Git side cache over fixture repositories.

use std::io::Read;
use std::sync::Arc;

use sidewise_core::{Location, ProviderPath};
use sidewise_engine::model::{CancelToken, EntryKind, Reason, RelPath};
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

#[test]
fn same_key_shares_one_side() {
    let fixture = fixture();
    let sides = GitSides::default();
    let first = sides.side(&at(&fixture, "main")).expect("open");
    let second = sides.side(&at(&fixture, "main")).expect("hit");
    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn ninth_key_drops_the_first() {
    let fixture = fixture();
    let sides = GitSides::default();
    let first = sides.side(&at(&fixture, "main")).expect("open");
    for n in 0..GIT_SIDES {
        let branch = format!("b{n}");
        let _ = fixture.commit(&branch, &[("f", b"x")]);
        sides.side(&at(&fixture, &branch)).expect("open");
    }
    let again = sides.side(&at(&fixture, "main")).expect("reopen");
    assert!(!Arc::ptr_eq(&first, &again));
}

#[test]
fn local_location_is_local_target() {
    let target = target_for(Arc::new(GitSides::default()));
    let location = Location::local(ProviderPath::from_text("/tmp")).expect("local");
    assert!(matches!(target(&location), CompareTarget::Local(_)));
}

#[test]
fn git_root_opens_and_lists() {
    let fixture = fixture();
    let target = target_for(Arc::new(GitSides::default()));
    let root = encode(fixture.path(), "main", Form::Stored).expect("encode");
    let CompareTarget::Side(open) = target(&root) else {
        panic!("a Git root is an opener");
    };
    let side = open().expect("open");
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
    assert_eq!(names, ["a.txt", "src"]);
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
    let side = side_fn(Arc::new(GitSides::default()));
    let bytes = file_at(&side, &fixture, "main", "src/lib.rs").expect("read");
    assert_eq!(bytes, "fn main() {}\n");
}

#[test]
fn side_fn_bad_rev_is_not_found() {
    let fixture = fixture();
    let side = side_fn(Arc::new(GitSides::default()));
    assert_eq!(
        file_at(&side, &fixture, "nope", "src/lib.rs").err(),
        Some(Reason::NotFound)
    );
}
