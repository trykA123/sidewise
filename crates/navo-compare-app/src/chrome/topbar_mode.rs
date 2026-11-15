//! Action variants and prototype pill definitions shared inside the topbar module.

/// Which topbar action fired; the shell registers one callback for all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Search,
    Theme,
    Accounts,
    View,
    Keys,
    Sync,
}

/// One action pill: its element id, its label, its key hint and the action it fires.
pub(super) struct Pill {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) hint: Option<&'static str>,
    pub(super) action: Action,
}

/// The action pills, in prototype order after the search filter.
#[rustfmt::skip]
pub(super) const PILLS: [Pill; 5] = [
    Pill { id: "top-theme", label: "Theme", hint: Some("T"), action: Action::Theme },
    Pill { id: "top-accounts", label: "Accounts", hint: None, action: Action::Accounts },
    Pill { id: "top-view", label: "View", hint: Some("Alt+V"), action: Action::View },
    Pill { id: "top-keys", label: "Keys", hint: Some("?"), action: Action::Keys },
    Pill { id: "top-sync", label: "Sync", hint: Some("Alt+S"), action: Action::Sync },
];
