//! The Start screen's Git probe: opens the typed repository and resolves its revision.
//! It blocks, so `start::probe` calls it only from the off-thread side check (NC195).

use navo_compare_cli::args::SideSpec;

use crate::start::SideHint;

/// Resolves a `repo@rev` side: `GitRef` when the revision names one commit, else the problem.
pub(crate) fn git_hint(spec: SideSpec) -> SideHint {
    let SideSpec::GitRev { repo, rev } = &spec else {
        return SideHint::NotFound;
    };
    match sidewise_git::repo::probe(repo, rev) {
        Ok(_) => SideHint::GitRef(spec),
        Err(problem) => SideHint::GitProblem(problem),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::start_tests::TempDir;
    use sidewise_git::fixture::Fixture;
    use sidewise_git::repo::GitProblem;

    fn spec(repo: &std::path::Path, rev: &str) -> SideSpec {
        SideSpec::GitRev {
            repo: repo.to_path_buf(),
            rev: rev.into(),
        }
    }

    #[test]
    fn a_resolving_revision_is_a_git_ref() {
        let fixture = Fixture::new();
        let _ = fixture.commit("main", &[("a.txt", b"one\n")]);
        let side = spec(fixture.dir.path(), "main");
        assert_eq!(git_hint(side.clone()), SideHint::GitRef(side));
    }

    #[test]
    fn an_unknown_revision_is_a_problem() {
        let fixture = Fixture::new();
        let _ = fixture.commit("main", &[("a.txt", b"one\n")]);
        let hint = git_hint(spec(fixture.dir.path(), "nope"));
        assert_eq!(hint, SideHint::GitProblem(GitProblem::NoSuchRevision));
    }

    #[test]
    fn a_plain_folder_is_not_a_repository() {
        let dir = TempDir::new("plainfolder");
        let hint = git_hint(spec(dir.path(), "main"));
        assert_eq!(hint, SideHint::GitProblem(GitProblem::NotRepository));
    }
}
