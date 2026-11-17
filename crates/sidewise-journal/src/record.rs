//! The journal record codec (NC143): one record per log line, `<compact json>\t<16 hex>\n`,
//! where the hex is the first 8 bytes of the blake3 hash of the json.

use navo_core::FileStamp;
use serde::{Deserialize, Serialize};

/// The id of one job, `{created_ms:013}-{pid}-{seq}`; it is also the job's folder name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobId(String);

impl JobId {
    /// Builds the id from the creation time (ms since the Unix epoch), the process id and the
    /// per-process job counter.
    pub fn new(created_ms: u64, pid: u32, seq: u64) -> Self {
        Self(format!("{created_ms:013}-{pid}-{seq}"))
    }

    /// The id as text, as written in the log and used as the folder name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a job does, as the user would name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    Copy,
    CopyMany,
    Undo,
    Sync,
}

/// What one item of a job does to its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemMode {
    Overwrite,
    Create,
    Remove,
}

/// One line of a job log. The variants and their order in a log follow NC143 and NC144.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Record {
    /// The first line of every job. `owner` is the stem of the owner lock, `<pid>-<start_ms>`.
    Begin {
        job: JobId,
        kind: JobKind,
        label: String,
        created_ms: u64,
        reverts: Option<JobId>,
        owner: String,
    },
    /// An item is about to be written; `target` is an absolute UTF-8 OS path.
    Intent {
        item: u32,
        target: String,
        mode: ItemMode,
        expect_size: Option<u64>,
        expect_mtime_ms: Option<i64>,
        restore_mtime_ms: Option<i64>,
    },
    /// The new bytes are staged and synced beside the target.
    Staged { item: u32, new: FileStamp },
    /// The old bytes are in a synced backup.
    Backed { item: u32, old: FileStamp },
    /// The target holds the new bytes.
    Done { item: u32 },
    /// The commit point: the job is complete.
    Commit,
    /// The job was rolled back; the log is closed.
    Aborted { reason: String },
}

/// The records of one log, in order, and what was wrong with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Log {
    /// Every good line before the first bad one.
    pub records: Vec<Record>,
    /// The last line had no `\n`; it was ignored.
    pub torn_tail: bool,
    /// A line that ends in `\n` failed its check; nothing after it was read.
    pub damaged: bool,
}

/// Hex digits of the checksum: 8 bytes of blake3, lowercase.
const CHECK_LEN: usize = 16;

/// Encodes one record as a log line that ends in `\n`. The line is pure ASCII, so a crash
/// that cuts it at any byte still leaves valid UTF-8 for `decode`.
pub fn encode(record: &Record) -> String {
    let json = ascii_json(record);
    let check = checksum(&json);
    format!("{json}\t{check}\n")
}

/// Decodes a whole log; it never fails, a bad log is reported in the flags of `Log`.
/// A last line with no `\n` is a torn tail and is ignored, even when its checksum holds:
/// a line counts only once its newline is written. Reading stops at the first bad line.
pub fn decode(text: &str) -> Log {
    let mut log = Log::default();
    let mut rest = text;
    while !rest.is_empty() {
        let Some((line, after)) = rest.split_once('\n') else {
            log.torn_tail = true;
            break;
        };
        match decode_line(line) {
            Some(record) => log.records.push(record),
            None => {
                log.damaged = true;
                break;
            }
        }
        rest = after;
    }
    log
}

/// Decodes one line without its `\n`; `None` when its shape, checksum or json is wrong.
fn decode_line(line: &str) -> Option<Record> {
    let (json, check) = line.rsplit_once('\t')?;
    let well_formed = check.len() == CHECK_LEN
        && check
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !well_formed || checksum(json) != check {
        return None;
    }
    serde_json::from_str(json).ok()
}

/// The first 8 bytes of the blake3 hash of `json`, as 16 lowercase hex digits.
fn checksum(json: &str) -> String {
    let hash = blake3::hash(json.as_bytes());
    let mut hex = String::with_capacity(CHECK_LEN);
    for byte in &hash.as_bytes()[..CHECK_LEN / 2] {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Serialises a record as compact json, with every non-ASCII char escaped as `\uXXXX`
/// (UTF-16 units, so a char past U+FFFF becomes a surrogate pair).
#[expect(
    clippy::expect_used,
    reason = "a Record has only string keys and plain fields, so serde_json cannot fail on it"
)]
fn ascii_json(record: &Record) -> String {
    let raw = serde_json::to_string(record).expect("a Record always serialises to json");
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii() {
            out.push(ch);
            continue;
        }
        let mut units = [0_u16; 2];
        for unit in ch.encode_utf16(&mut units) {
            out.push_str(&format!("\\u{unit:04x}"));
        }
    }
    out
}
