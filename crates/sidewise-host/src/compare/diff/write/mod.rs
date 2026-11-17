//! The write side of the file diff in the host: copying changes into files through the journal.
//!
//! `request` holds the port of the copy request and its refusals; `compose` writes the copied
//! result into a fill writer; the host follows.

pub mod compose;
pub mod request;
