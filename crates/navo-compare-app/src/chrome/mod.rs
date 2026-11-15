//! Window chrome around the session: the sidebar and the topbar today, the status bar later.

pub mod sidebar;
pub mod status_bar;
#[cfg(test)]
mod status_bar_tests;
#[cfg(test)]
mod tests;
pub mod topbar_status;
