# Changelog

This file follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Folders shows both sides in one row: name, size and time on each side, with a status column between them. Narrow windows show names only.
- Mouse: double click opens a file's diff or a folder; the arrow opens a folder; middle click closes a tab.
- Right-click menus on Folders rows, Recent rows and tabs. `Shift+F10` opens the same menu from the keyboard.
- Folders menu: Open diff, Open in Stream, Copy left or right path, Copy name, Expand all, and Reveal in Explorer.
- Recent menu: Open, Copy paths, and Remove from Recent with Undo.
- Tab menu: Close tab and Close other tabs.
- The View menu (`Alt+V`) on Start and Folders: Simple, Rich or Custom, side by side or unified, changed words and wrap. Your choice is saved in the profile.
- Pasted paths in double quotes (Explorer's "Copy as path") work on the Start screen.

### Changed

- A click now selects a folder; double click or the arrow opens it.

## [0.1.0-alpha.1] - 2026-11-16

The first public pre-release of Sidewise. It compares two folders or two files on 64-bit Windows.
It is read-only. It never changes your files.

### Added

- A Start screen to pick two folders or two files. Each compare opens in its own tab.
- Recent sessions on the Start screen. They are stored in `%LOCALAPPDATA%\Sidewise\sessions`.
- A progress view for large compares. It shows the progress of each phase.
- A Folders view with a folder tree, filters and a change map.
- A file diff for text, hex and images.
- A Stream view with an Outline. The compare engine comes from Navo.
- A Keys sheet that lists the keyboard shortcuts.
- Toast messages for short notices and errors.
- Light and Dark mode. Sidewise follows the Windows mode and uses the Violet colour scheme.
- A command line: `sidewise <left> <right>`, `--profile DIR`, `--version` and `--help`.
- A portable zip with one `sidewise.exe`. There is no installer.

### Known limits

- Windows only. There is no installer.
- The build is not signed. Windows SmartScreen warns on the first start. Choose **More info**, then **Run anyway**.
- Read-only. Sidewise does not copy, merge or sync files.
- No Git branch or ref compare. A Git side exits with code 2 on the command line.
- Command line errors exit with code 2. This includes one side only, `--select-left` and `--compare-with`.
- Some flags, such as `--wait`, are accepted and ignored.
- Each start opens its own window.
- No accounts and no theme switching. The colour scheme follows Windows Light or Dark mode only.
- No View menu of its own. Inside a file diff, `Alt+V` opens the diff's menu.
- No Sync plan, no copying of changes and no three-way merge.
- No Explorer menu and no auto-update.
- In a narrow window the hex view of one file can draw its text column over the right side. The same bytes show correctly in Stream.
