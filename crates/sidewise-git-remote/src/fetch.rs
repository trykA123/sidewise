//! Fetches one remote ref, shallow (depth 1), into a bare cache repository: one connection lists
//! the candidate refs, a second fetches the chosen one, both over a capped `Http` backend (NC220).

use std::num::NonZeroU32;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use gix::bstr::BStr;
use gix::progress::Discard;
use gix::remote::{Direction, fetch::Shallow, fetch::Tags, ref_map};
use gix_transport::client::blocking_io::http::Http;

use crate::lent::lend;
use crate::{Budget, Capped, DeadlineGuard, Limits, RemoteProblem, RemoteRev, RemoteUrl};
use crate::{candidates, choose, problem_of};

/// The open options of a cache repository: no global, system or environment config (NC219),
/// and the per-object and thread caps of NC221.
fn open_options() -> gix::open::Options {
    gix::open::Options::isolated()
        .cli_overrides(["gitoxide.objects.allocLimit=67108864", "pack.threads=4"])
}

/// The local ref a revision is fetched to: `refs/sidewise/<16 hex of blake3(rev)>`.
fn local_ref(rev: &RemoteRev) -> String {
    let hash = blake3::hash(rev.as_str().as_bytes());
    format!("refs/sidewise/{}", &hash.to_hex()[..16])
}

/// Fetches `rev` of `url` into the bare repository at `repo_dir` (created when missing or empty)
/// through `http`, within `limits`. Returns the local ref name; a panic inside is `Broken`.
#[expect(
    clippy::too_many_arguments,
    reason = "the signature is fixed by plan row RG5b; RG6a calls it as is"
)]
pub fn fetch_with<H: Http>(
    repo_dir: &Path,
    url: &RemoteUrl,
    rev: &RemoteRev,
    http: H,
    limits: Limits,
) -> Result<String, RemoteProblem> {
    let run = AssertUnwindSafe(|| run(&Job::open(repo_dir, url, rev)?, http, limits));
    catch_unwind(run).unwrap_or(Err(RemoteProblem::Broken))
}

/// Opens the cache repository, or creates it when `dir` is missing or empty.
fn open_or_init(dir: &Path) -> gix::Result<gix::Repository> {
    let empty = std::fs::read_dir(dir).map_or(true, |mut entries| entries.next().is_none());
    if empty {
        let kind = gix::create::Kind::Bare;
        let options = gix::create::Options::default();
        let repo = gix::ThreadSafeRepository::init_opts(dir, kind, options, open_options())?;
        return Ok(repo.to_thread_local());
    }
    gix::open_opts(dir, open_options())
}

/// One fetch: the open cache repository, the remote address, the rev and its local ref.
struct Job<'a> {
    repo: gix::Repository,
    url: gix::Url,
    rev: &'a RemoteRev,
    local: String,
}

impl<'a> Job<'a> {
    /// Checks the rev again (`NoSuchRevision`), then opens the repository (`Broken` on failure).
    fn open(dir: &Path, url: &RemoteUrl, rev: &'a RemoteRev) -> Result<Self, RemoteProblem> {
        if gix::validate::reference::name_partial(BStr::new(rev.as_str())).is_err() {
            return Err(RemoteProblem::NoSuchRevision);
        }
        let repo = open_or_init(dir).map_err(|_| RemoteProblem::Broken)?;
        let url = gix::url::parse(BStr::new(url.as_str())).map_err(|_| RemoteProblem::Broken)?;
        let local = local_ref(rev);
        Ok(Self {
            repo,
            url,
            rev,
            local,
        })
    }
}

/// Runs both connections of `job` under one byte budget and deadline timer; maps the failure.
fn run<H: Http>(job: &Job<'_>, http: H, limits: Limits) -> Result<String, RemoteProblem> {
    let budget = Arc::new(Budget::new(limits));
    let interrupt = Arc::new(AtomicBool::new(false));
    let guard = DeadlineGuard::start(Arc::clone(&budget), Arc::clone(&interrupt))
        .map_err(|_| RemoteProblem::Broken)?;
    let mut capped = Capped::new(http, Arc::clone(&budget));
    let result = fetch_ref(job, &mut capped, &interrupt);
    // The join inside the drop makes the timer's `TimedOut` visible to `problem_of`.
    drop(guard);
    let fetched = result.map_err(|err| problem_of(&err, &budget))?;
    fetched.map(|()| job.local.clone())
}

/// Connection 1: the names the server lists for the candidates of the rev (ls-refs prefixes).
fn list<C: Http>(
    remote: &gix::Remote<'_>,
    job: &Job<'_>,
    http: &mut C,
) -> gix::Result<Vec<String>> {
    let mut extra = Vec::new();
    for name in candidates(job.rev) {
        let spec = gix::refspec::parse(BStr::new(&name), gix::refspec::parse::Operation::Fetch)?;
        extra.push(spec.to_owned());
    }
    let options = ref_map::Options {
        prefix_from_spec_as_filter_on_remote: true,
        extra_refspecs: extra,
        ..Default::default()
    };
    let (map, _) = remote
        .to_connection_with_transport(lend(http, &job.url))
        .with_credentials(|_| Ok(None))
        .ref_map(Discard, options)?;
    let names = map.remote_refs.iter().map(|r| r.unpack().0.to_string());
    Ok(names.collect())
}

/// Lists, chooses, then fetches the chosen ref at depth 1 to the job's local ref (connection 2).
/// The outer error is gix's; the inner one a problem found here (no such rev, ref not written).
fn fetch_ref<C: Http>(
    job: &Job<'_>,
    http: &mut C,
    interrupt: &AtomicBool,
) -> gix::Result<Result<(), RemoteProblem>> {
    let remote = job
        .repo
        .remote_at_without_url_rewrite(job.url.clone())?
        .with_fetch_tags(Tags::None);
    let full = match choose(&list(&remote, job, http)?, job.rev) {
        Ok(full) => full,
        Err(problem) => return Ok(Err(problem)),
    };
    let spec = format!("+{full}:{}", job.local);
    let remote = remote.with_refspecs([spec.as_str()], Direction::Fetch)?;
    remote
        .to_connection_with_transport(lend(http, &job.url))
        .with_credentials(|_| Ok(None))
        .prepare_fetch(Discard, ref_map::Options::default())?
        .with_shallow(Shallow::DepthAtRemote(NonZeroU32::MIN))
        .receive(Discard, interrupt)?;
    let written = job.repo.try_find_reference(job.local.as_str())?.is_some();
    Ok(written.then_some(()).ok_or(RemoteProblem::Broken))
}
