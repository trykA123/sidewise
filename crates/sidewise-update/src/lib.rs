//! Sidewise's update check: which release is newer, the GitHub feed and the request. No GUI.
// Windows allows unsafe code only in `src/winhttp.rs`, function by function (workspace lint: deny).
#![cfg_attr(not(windows), forbid(unsafe_code))]

pub mod check;
pub mod feed;
pub mod fetch;
pub mod version;
