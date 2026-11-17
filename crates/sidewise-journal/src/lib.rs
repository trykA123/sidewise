//! The write journal of Sidewise (plan P1, NC143): every write is logged as a job of
//! checksummed records, so a crash can be rolled back and every write can be undone.

pub mod record;

#[cfg(test)]
mod codec_strict_tests;
#[cfg(test)]
mod codec_tests;

pub use record::{ItemMode, JobId, JobKind, Log, Record, decode, encode};
