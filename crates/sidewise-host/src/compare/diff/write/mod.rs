//! The write side of the file diff in the host: copying changes into files through the journal.
//!
//! `request` holds the port of the copy request and its refusals; the stage and host follow.

pub mod request;
