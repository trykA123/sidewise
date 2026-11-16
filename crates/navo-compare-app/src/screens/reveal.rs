//! Reveal in Explorer (NC138): builds the one argument for `explorer.exe /select,` from a
//! checked path and spawns Explorer on Windows. Used by the Folders context menu.

use std::path::Path;

/// The toast text off Windows, where there is no Explorer to open.
pub const OFF_WINDOWS: &str = "Reveal in Explorer works on Windows only.";

/// The `/select,"<path>"` argument for `explorer.exe`. Fails with a reason when the path is not
/// absolute, does not exist, is not UTF-8, or holds `"` or a control character (U+0000-U+001F,
/// U+007F), because the text goes to Explorer unescaped.
pub fn select_arg(path: &Path) -> Result<String, String> {
    if !path.is_absolute() {
        return Err("the path is not absolute".to_owned());
    }
    let text = path.to_str().ok_or("the path is not valid UTF-8")?;
    if text.chars().any(|c| c == '"' || c.is_control()) {
        return Err("the path holds a quote or a control character".to_owned());
    }
    if !path.exists() {
        return Err("the path does not exist".to_owned());
    }
    Ok(format!("/select,\"{text}\""))
}

/// Opens Explorer with `path` selected and does not wait for it. Fails with the user-facing
/// toast text: a refused path, a spawn error, or the off-Windows notice.
#[cfg(windows)]
pub fn reveal(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt as _;
    let arg = select_arg(path).map_err(|why| format!("Could not open Explorer: {why}."))?;
    std::process::Command::new("explorer.exe")
        .raw_arg(arg)
        .spawn()
        .map(drop)
        .map_err(|error| format!("Could not open Explorer: {error}."))
}

/// Off Windows there is no Explorer: always the notice.
#[cfg(not(windows))]
pub fn reveal(_path: &Path) -> Result<(), String> {
    Err(OFF_WINDOWS.to_owned())
}

#[cfg(test)]
#[path = "reveal_tests.rs"]
mod tests;
