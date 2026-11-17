//! Durable copies for the app (P1 A1): opens the write journal once at start, keeps the copy
//! host for the process, and words the start-up toast (recovery note or "Copying is off").

use std::path::Path;
use std::sync::{Arc, OnceLock};

use navo_compare_profile::book::profile_text;
use navo_compare_profile::paths::open_profile;
use sidewise_fs::durable::RealDisk;
use sidewise_host::compare::diff::write::host::LocalCopyHost;
use sidewise_journal::{Journal, Recovered};
use sidewise_kit::prelude::App;

use crate::layers::toast;

/// The process copy host; unset when the journal did not open, so copies stay refused (NC142).
static HOST: OnceLock<Arc<LocalCopyHost>> = OnceLock::new();

/// Opens the journal in the profile (`explicit` folder, else the platform folders) and installs
/// the copy host. `Ok(Some(note))` when recovery restored, kept or failed anything;
/// `Err("Copying is off: <reason>")` when the profile or the journal did not open (fail closed).
pub(crate) fn install_writes(explicit: Option<&Path>) -> Result<Option<String>, String> {
    if HOST.get().is_some() {
        return Ok(None);
    }
    let (host, note) = open_writes(explicit)?;
    // A lost race keeps the first host; this one drops and releases its owner lock.
    let _ = HOST.set(host);
    Ok(note)
}

/// The copy host, when the journal opened at start.
pub(crate) fn host() -> Option<Arc<LocalCopyHost>> {
    HOST.get().cloned()
}

/// Opens the profile and its journal (recovery and retention sweep run here) and returns a new
/// copy host with the recovery note; fails with "Copying is off: <reason>".
pub(crate) fn open_writes(
    explicit: Option<&Path>,
) -> Result<(Arc<LocalCopyHost>, Option<String>), String> {
    let profile = open_profile(explicit).map_err(|error| off(&profile_text(&error)))?;
    let dir = profile.journal_dir();
    let (journal, recovered) = Journal::open(&dir, Arc::new(RealDisk)).map_err(|error| {
        tracing::warn!(dir = %dir.display(), %error, "journal did not open; copying is off");
        off(&error.to_string())
    })?;
    if recovered.swept.failed > 0 {
        tracing::warn!(
            failed = recovered.swept.failed,
            bytes_left = recovered.swept.bytes_left,
            "journal retention sweep left jobs for the next start"
        );
    }
    let host = Arc::new(LocalCopyHost::new(Arc::new(journal)));
    Ok((host, recovery_note(&recovered, &dir)))
}

/// Toasts the outcome of `install_writes` once the shell is up: the recovery note or the
/// "Copying is off" reason; nothing for a clean start.
pub(crate) fn announce(outcome: Result<Option<String>, String>, cx: &mut App) {
    if let Ok(Some(text)) | Err(text) = outcome {
        toast::notify(text, cx);
    }
}

/// "Copying is off: <reason>".
fn off(reason: &str) -> String {
    format!("Copying is off: {reason}")
}

/// The recovery toast: files restored after an interrupted copy, then what was left and the
/// journal folder that keeps its backups. `None` when recovery found nothing to report.
pub(crate) fn recovery_note(recovered: &Recovered, dir: &Path) -> Option<String> {
    let mut parts = Vec::new();
    if recovered.restored > 0 {
        parts.push(format!(
            "Sidewise restored {} after an interrupted copy",
            files(recovered.restored)
        ));
    }
    if recovered.kept > 0 {
        parts.push(format!(
            "{} changed since the interrupted copy and {} kept as is",
            files(recovered.kept),
            if recovered.kept == 1 { "was" } else { "were" }
        ));
    }
    if recovered.failed > 0 {
        parts.push(format!(
            "{} could not be restored now; Sidewise tries again at the next start",
            files(recovered.failed)
        ));
    }
    if recovered.damaged > 0 {
        parts.push(format!(
            "{} damaged journal {} left untouched",
            recovered.damaged,
            if recovered.damaged == 1 {
                "entry was"
            } else {
                "entries were"
            }
        ));
    }
    if recovered.kept > 0 || recovered.failed > 0 || recovered.damaged > 0 {
        parts.push(format!("The backups stay in {}", dir.display()));
    }
    (!parts.is_empty()).then(|| parts.join(". "))
}

/// "1 file" or "<n> files".
fn files(count: u32) -> String {
    if count == 1 {
        "1 file".to_owned()
    } else {
        format!("{count} files")
    }
}
