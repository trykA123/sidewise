//! Remote Git sides for Sidewise; today only the `git-remote` location form.
#![forbid(unsafe_code)]

pub mod at;
pub mod capped;
pub mod choose;
pub mod failure;
pub mod fetch;
mod lent;
pub mod problem;

pub use at::{RemoteAt, RemoteAtError};
pub use capped::{Budget, Capped, CappedRead, DeadlineGuard, Limits};
pub use choose::{candidates, choose};
pub use failure::problem_of;
pub use fetch::fetch_with;
pub use problem::{HttpStatus, NetworkFailure, RemoteProblem};
pub use sidewise_git_address::{AddressError, RemoteRev, RemoteUrl};
