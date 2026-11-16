//! Screens that the shell's tabs show: the compare run model today, the compare tab later.
//! Each screen owns its state and reports changes through the shell.

pub mod compare_host;
#[cfg(test)]
mod compare_host_tests;
pub mod compare_run;
#[cfg(test)]
mod compare_run_phase_tests;
pub mod compare_tab;
mod compare_tab_events;
pub mod compare_tab_keys;
pub mod folders_model;
#[cfg(test)]
mod folders_model_tests;
mod folders_row;
pub mod folders_view;
pub mod large_model;
#[cfg(test)]
mod large_model_tests;
pub mod large_view;
#[cfg(test)]
mod large_view_tests;
pub mod pair_requests;
#[cfg(test)]
mod pair_requests_tests;
