//! Fetch caps: a byte and time `Budget`, the `Capped` wrapper every `Http` backend runs inside,
//! and the `DeadlineGuard` timer that interrupts gix at the deadline (NC221).

use std::io::{self, BufRead, Read};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use gix_transport::client::blocking_io::http::{GetResponse, Http, PostBodyDataKind, PostResponse};

use crate::RemoteProblem;

/// The caps of one fetch: response bytes and wall-clock time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_bytes: u64,
    pub deadline: Duration,
}

impl Limits {
    /// The product caps: 256 MiB of response bytes and 300 s.
    pub const PRODUCT: Self = Self {
        max_bytes: 256 * 1024 * 1024,
        deadline: Duration::from_secs(300),
    };
}

/// The state of one fetch's caps, shared through an `Arc` by `Capped` and `DeadlineGuard`.
#[derive(Debug)]
pub struct Budget {
    limits: Limits,
    used: AtomicU64,
    start: Instant,
    problem: OnceLock<RemoteProblem>,
}

impl Budget {
    /// Starts a budget now with `limits`.
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            used: AtomicU64::new(0),
            start: Instant::now(),
            problem: OnceLock::new(),
        }
    }

    /// The cap that stopped the fetch, if any; the first one set wins.
    pub fn problem(&self) -> Option<RemoteProblem> {
        self.problem.get().copied()
    }

    /// Records `problem` (unless one is already set) and returns the read error for it.
    fn fail(&self, problem: RemoteProblem) -> io::Error {
        let _ = self.problem.set(problem);
        let (kind, text) = match self.problem() {
            Some(RemoteProblem::TimedOut) => (io::ErrorKind::TimedOut, "fetch deadline passed"),
            _ => (io::ErrorKind::FileTooLarge, "response over the byte cap"),
        };
        io::Error::new(kind, text)
    }

    /// The time left before the deadline (zero once it has passed).
    fn remaining(&self) -> Duration {
        self.limits.deadline.saturating_sub(self.start.elapsed())
    }

    /// Counts `bytes` more response bytes; fails when the total goes over `max_bytes`.
    pub(crate) fn charge(&self, bytes: usize) -> io::Result<()> {
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        let total = self
            .used
            .fetch_add(bytes, Ordering::SeqCst)
            .saturating_add(bytes);
        if total > self.limits.max_bytes {
            return Err(self.fail(RemoteProblem::TooLarge));
        }
        Ok(())
    }
}

/// A response reader that charges every byte it hands out to a `Budget` and stops at its caps.
pub struct CappedRead<R> {
    inner: R,
    budget: Arc<Budget>,
    /// Bytes of the inner buffer already charged but not yet consumed.
    charged: usize,
}

impl<R: BufRead> BufRead for CappedRead<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.budget.remaining().is_zero() {
            return Err(self.budget.fail(RemoteProblem::TimedOut));
        }
        let buf = self.inner.fill_buf()?;
        if buf.len() > self.charged {
            self.budget.charge(buf.len() - self.charged)?;
            self.charged = buf.len();
        }
        Ok(buf)
    }

    fn consume(&mut self, amount: usize) {
        self.charged = self.charged.saturating_sub(amount);
        self.inner.consume(amount);
    }
}

impl<R: BufRead> Read for CappedRead<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let buf = self.fill_buf()?;
        let n = buf.len().min(out.len());
        out[..n].copy_from_slice(&buf[..n]);
        self.consume(n);
        Ok(n)
    }
}

/// An `Http` backend wrapped so that its response headers and bodies obey one `Budget`.
pub struct Capped<H> {
    inner: H,
    budget: Arc<Budget>,
}

impl<H> Capped<H> {
    /// Wraps `inner`; every response it returns is charged to `budget`.
    pub fn new(inner: H, budget: Arc<Budget>) -> Self {
        Self { inner, budget }
    }

    /// Wraps one response reader in the budget.
    fn wrap<R>(&self, inner: R) -> CappedRead<R> {
        CappedRead {
            inner,
            budget: Arc::clone(&self.budget),
            charged: 0,
        }
    }
}

impl<H: Http> Http for Capped<H> {
    type Headers = CappedRead<H::Headers>;
    type ResponseBody = CappedRead<H::ResponseBody>;
    type PostBody = H::PostBody;

    fn get(
        &mut self,
        url: &str,
        base_url: &str,
        headers: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> gix::Result<GetResponse<Self::Headers, Self::ResponseBody>> {
        let response = self.inner.get(url, base_url, headers)?;
        Ok(GetResponse {
            headers: self.wrap(response.headers),
            body: self.wrap(response.body),
        })
    }

    fn post(
        &mut self,
        url: &str,
        base_url: &str,
        headers: impl IntoIterator<Item = impl AsRef<str>>,
        body: PostBodyDataKind,
    ) -> gix::Result<PostResponse<Self::Headers, Self::ResponseBody, Self::PostBody>> {
        let response = self.inner.post(url, base_url, headers, body)?;
        Ok(PostResponse {
            post_body: response.post_body,
            headers: self.wrap(response.headers),
            body: self.wrap(response.body),
        })
    }

    fn configure(&mut self, config: &dyn std::any::Any) -> gix::Result {
        self.inner.configure(config)
    }

    fn redirected_base_url(&self) -> Option<String> {
        self.inner.redirected_base_url()
    }
}

/// A timer thread that records `TimedOut` and sets gix's interrupt flag at the budget's deadline.
/// Drop it before reading the budget's problem: dropping stops the thread and joins it.
pub struct DeadlineGuard {
    timer: Option<(mpsc::Sender<()>, JoinHandle<()>)>,
}

impl DeadlineGuard {
    /// Starts the timer for `budget`; at the deadline it sets `interrupt`.
    /// Fails when the system cannot start a thread.
    pub fn start(budget: Arc<Budget>, interrupt: Arc<AtomicBool>) -> io::Result<Self> {
        let (stop, stopped) = mpsc::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("sidewise-fetch-deadline".into())
            .spawn(move || {
                // Only a timeout fires; a dropped sender (Disconnected) means the fetch ended first.
                if let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(budget.remaining()) {
                    let _ = budget.fail(RemoteProblem::TimedOut);
                    interrupt.store(true, Ordering::SeqCst);
                }
            })?;
        Ok(Self {
            timer: Some((stop, thread)),
        })
    }
}

impl Drop for DeadlineGuard {
    fn drop(&mut self) {
        if let Some((stop, thread)) = self.timer.take() {
            drop(stop);
            // The timer body cannot panic; a join error leaves nothing to clean up.
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "test: a failed step should abort the test"
    )]

    use super::*;
    use std::io::{BufReader, Cursor, Sink};

    /// An in-memory `Http`: every response is `headers` then `body`, read 4 bytes at a time.
    struct Fake {
        headers: Vec<u8>,
        body: Vec<u8>,
    }

    type Reader = BufReader<Cursor<Vec<u8>>>;

    impl Fake {
        fn reader(bytes: &[u8]) -> Reader {
            BufReader::with_capacity(4, Cursor::new(bytes.to_vec()))
        }
    }

    impl Http for Fake {
        type Headers = Reader;
        type ResponseBody = Reader;
        type PostBody = Sink;

        fn get(
            &mut self,
            _: &str,
            _: &str,
            _: impl IntoIterator<Item = impl AsRef<str>>,
        ) -> gix::Result<GetResponse<Reader, Reader>> {
            Ok(GetResponse {
                headers: Self::reader(&self.headers),
                body: Self::reader(&self.body),
            })
        }

        fn post(
            &mut self,
            url: &str,
            base: &str,
            headers: impl IntoIterator<Item = impl AsRef<str>>,
            _: PostBodyDataKind,
        ) -> gix::Result<PostResponse<Reader, Reader, Sink>> {
            let got = self.get(url, base, headers)?;
            Ok(PostResponse {
                post_body: io::sink(),
                headers: got.headers,
                body: got.body,
            })
        }

        fn configure(&mut self, _: &dyn std::any::Any) -> gix::Result {
            Ok(())
        }
    }

    fn capped(headers: usize, body: usize, limits: Limits) -> (Capped<Fake>, Arc<Budget>) {
        let budget = Arc::new(Budget::new(limits));
        let fake = Fake {
            headers: vec![b'h'; headers],
            body: vec![b'b'; body],
        };
        (Capped::new(fake, Arc::clone(&budget)), budget)
    }

    fn limits(max_bytes: u64, deadline_ms: u64) -> Limits {
        Limits {
            max_bytes,
            deadline: Duration::from_millis(deadline_ms),
        }
    }

    #[test]
    fn product_limits() {
        assert_eq!(Limits::PRODUCT.max_bytes, 268_435_456);
        assert_eq!(Limits::PRODUCT.deadline, Duration::from_secs(300));
    }

    #[test]
    fn body_within_cap_reads_whole() {
        let (mut http, budget) = capped(10, 30, limits(40, 60_000));
        let mut got = http.get("u", "b", ["x"]).unwrap();
        let mut out = Vec::new();
        got.headers.read_to_end(&mut out).unwrap();
        got.body.read_to_end(&mut out).unwrap();
        assert_eq!(out.len(), 40);
        assert_eq!(budget.problem(), None);
    }

    #[test]
    fn body_over_cap_fails_too_large() {
        let (mut http, budget) = capped(10, 31, limits(40, 60_000));
        let mut got = http
            .post("u", "b", ["x"], PostBodyDataKind::Unbounded)
            .unwrap();
        let mut out = Vec::new();
        got.headers.read_to_end(&mut out).unwrap();
        let err = got.body.read_to_end(&mut out).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::FileTooLarge);
        assert!(
            out.len() <= 40,
            "handed out {} bytes over a 40-byte cap",
            out.len()
        );
        assert_eq!(budget.problem(), Some(RemoteProblem::TooLarge));
    }

    #[test]
    fn read_after_deadline_fails_timed_out() {
        let (mut http, budget) = capped(0, 64, limits(1 << 20, 40));
        let mut got = http.get("u", "b", ["x"]).unwrap();
        let mut chunk = [0; 4];
        assert_eq!(got.body.read(&mut chunk).unwrap(), 4);
        std::thread::sleep(Duration::from_millis(60));
        let err = got.body.read(&mut chunk).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert_eq!(budget.problem(), Some(RemoteProblem::TimedOut));
    }

    #[test]
    fn first_problem_wins() {
        let (mut http, budget) = capped(0, 64, limits(4, 0));
        let mut got = http.get("u", "b", ["x"]).unwrap();
        assert!(got.body.read(&mut [0; 8]).is_err());
        assert_eq!(budget.problem(), Some(RemoteProblem::TimedOut));
        let _ = budget.charge(100);
        assert_eq!(budget.problem(), Some(RemoteProblem::TimedOut));
    }

    #[test]
    fn guard_sets_interrupt_at_deadline() {
        let budget = Arc::new(Budget::new(limits(1, 50)));
        let flag = Arc::new(AtomicBool::new(false));
        let guard = DeadlineGuard::start(Arc::clone(&budget), Arc::clone(&flag)).unwrap();
        assert!(!flag.load(Ordering::SeqCst));
        let start = Instant::now();
        while !flag.load(Ordering::SeqCst) && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(flag.load(Ordering::SeqCst), "interrupt not set after 5 s");
        assert_eq!(budget.problem(), Some(RemoteProblem::TimedOut));
        drop(guard);
    }

    #[test]
    fn dropping_guard_ends_thread_without_interrupt() {
        let budget = Arc::new(Budget::new(limits(1, 3_600_000)));
        let flag = Arc::new(AtomicBool::new(false));
        let guard = DeadlineGuard::start(Arc::clone(&budget), Arc::clone(&flag)).unwrap();
        let start = Instant::now();
        drop(guard);
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "drop waited for the deadline"
        );
        assert_eq!(
            Arc::strong_count(&budget),
            1,
            "timer thread still holds the budget"
        );
        assert!(!flag.load(Ordering::SeqCst));
        assert_eq!(budget.problem(), None);
    }
}
