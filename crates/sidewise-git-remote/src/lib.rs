//! Remote Git sides for Sidewise; today only the `git-remote` location form.
#![forbid(unsafe_code)]

pub mod at;

pub use at::{RemoteAt, RemoteAtError};
pub use sidewise_git_address::{AddressError, RemoteRev, RemoteUrl};
