//! Reads local Git repositories for Sidewise, in process and read-only. Never starts a program.
#![forbid(unsafe_code)]

#[cfg(any(test, feature = "test-support"))]
pub mod fixture;
pub mod index;
pub mod location;
pub mod repo;
pub mod side;
