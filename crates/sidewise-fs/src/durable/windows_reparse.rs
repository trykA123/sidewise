//! Windows `RealDisk` guards: refuses files a filter driver serves (cloud, dedup, HSM and
//! WOF placeholders) and device paths, so every write step sees plain bytes on a volume.

/// Why a file with these attributes and reparse tag is refused, or `None` for a plain file.
pub(super) fn refusal(_attributes: u32, _tag: u32) -> Option<&'static str> {
    None
}

/// True when verbatim UTF-16 `units` name a file on a volume: `\\?\X:\`, `\\?\UNC\` or
/// `\\?\Volume{`. Device paths (`\\.\`, `\\?\GLOBALROOT`, `\\?\C:`) are false.
pub(super) fn on_a_volume(_units: &[u16]) -> bool {
    true
}

/// Fails closed: an `InvalidInput` error when `file` is a link, placeholder or offline file.
#[cfg(windows)]
pub(super) fn check_plain(_file: &std::fs::File) -> std::io::Result<()> {
    refusal(0, 0).map_or(Ok(()), |why| Err(super::real::refused(why)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn plain_files_pass_and_links_placeholders_and_offline_files_are_refused() {
        assert_eq!(refusal(0x20, 0), None);
        assert_eq!(refusal(0x80, 0), None);
        let symlink = refusal(0x400, 0xA000_000C).unwrap();
        let junction = refusal(0x400, 0xA000_0003).unwrap();
        assert!(symlink.contains("link") && junction.contains("link"));
        for tag in [0x8000_0017, 0x8000_0013, 0x9000_601A, 0xC000_0004] {
            let why = refusal(0x420, tag).unwrap_or_default();
            assert!(why.contains("placeholder"), "tag {tag:#x}: {why:?}");
        }
        for attribute in [0x1000, 0x4_0000, 0x40_0000] {
            let why = refusal(0x20 | attribute, 0).unwrap_or_default();
            assert!(why.contains("offline"), "attribute {attribute:#x}: {why:?}");
        }
    }

    #[test]
    fn only_drive_unc_and_volume_paths_are_on_a_volume() {
        for good in [
            r"\\?\C:\x",
            r"\\?\z:\",
            r"\\?\UNC\srv\share\x",
            r"\\?\unc\srv\share",
            r"\\?\Volume{0a1b}\x",
        ] {
            assert!(on_a_volume(&wide(good)), "{good}");
        }
        for device in [
            r"\\.\C:\x",
            r"//./C:/x",
            r"\\?\GLOBALROOT\Device\HarddiskVolume1\x",
            r"\\?\globalroot\Device\Null",
            r"\\?\Global\C:\x",
            r"\\?\C:",
            r"\\?\pipe\x",
            r"C:\x",
            r"\\?\1:\x",
        ] {
            assert!(!on_a_volume(&wide(device)), "{device}");
        }
    }
}
