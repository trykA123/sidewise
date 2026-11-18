//! Remote Git sides for Sidewise; today only the `git-remote` location form.
#![forbid(unsafe_code)]

pub mod at;
pub mod choose;
pub mod problem;

pub use at::{RemoteAt, RemoteAtError};
pub use choose::{candidates, choose};
pub use problem::{HttpStatus, NetworkFailure, RemoteProblem};
pub use sidewise_git_address::{AddressError, RemoteRev, RemoteUrl};
