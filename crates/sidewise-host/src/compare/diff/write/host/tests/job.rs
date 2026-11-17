//! Tests of the toast's Undo by job id (NC166): only the newest copy undoes; an older one is
//! refused as `NotNewest` and writes nothing.

use super::*;

impl Fixture {
    /// Undo copy `job` through the backend; the end, when it started.
    fn undo_job(&self, job: &str) -> (UndoStart, Option<UndoDone>) {
        let (sender, receiver) = mpsc::channel();
        let sink: UndoSink = Arc::new(move |done| {
            let _ = sender.send(done);
        });
        let start = self.backend.undo_last(Some(job), sink);
        if start != UndoStart::Started {
            assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());
            return (start, None);
        }
        (start, Some(receiver.recv_timeout(WAIT).unwrap()))
    }
}

#[test]
fn only_the_newest_copy_undoes_by_its_job_id() {
    let f = Fixture::new(
        b"a\nB\nc\nd\ne\nf\ng\nH\n",
        b"a\nb\nc\nd\ne\nf\ng\nh\n",
        Plan::Pass,
    );
    let mut changes = f.changes();
    changes.pop();
    let first = f.copy_done(changes).job.expect("first job id");
    let mut again = f.changes();
    let second = f
        .copy_done(vec![again.remove(0)])
        .job
        .expect("second job id");
    assert_ne!(first, second);
    assert_eq!(f.undo_job(&first), (UndoStart::NotNewest, None));
    assert_eq!(f.right(), b"a\nB\nc\nd\ne\nf\ng\nH\n", "nothing undone");
    let (start, done) = f.undo_job(&second);
    assert_eq!(start, UndoStart::Started);
    assert_eq!(done.unwrap().result, CopyResult::Copied);
    assert_eq!(f.right(), b"a\nB\nc\nd\ne\nf\ng\nh\n");
    assert_eq!(f.undo_job(&first).0, UndoStart::Started);
    assert_eq!(f.right(), b"a\nb\nc\nd\ne\nf\ng\nh\n");
    assert_eq!(
        f.undo_job(&second),
        (UndoStart::NotNewest, None),
        "already undone"
    );
}
