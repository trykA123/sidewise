# Changelog

This file follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed

- Closing the last tab leaves a New compare tab open instead of quitting.
- Copy commands confirm with a short message.

### Fixed

- The Keys sheet lists all viewer keys (line and page scrolling, find results, undo).
- The View menu closes on a click outside it and when you switch tabs.
- Closing a menu with Esc returns focus to where it was.

### Removed

- The Git compare entry in the sidebar; Git compare is not in this alpha.

## [0.1.0-alpha.2] — 2026-11-17

Sidewise now builds its compare engine, file layer and compare screen from its own crates (`crates/sidewise-*`, imported with their history from Navo). It no longer fetches Navo at build time.

### Added

- Folders shows both sides in one row: name, size and time on each side, with a status column between them. Narrow windows show names only.
- Mouse: double click opens a file's diff or a folder; the arrow opens a folder; middle click closes a tab.
- Right-click menus on Folders rows, Recent rows and tabs. `Shift+F10` opens the same menu from the keyboard.
- Folders menu: Open diff, Open in Stream, Copy left or right path, Copy name, Expand all, and Reveal in Explorer.
- Recent menu: Open, Copy paths, and Remove from Recent with Undo. It opens from the Start screen and from the Recent list in the sidebar.
- Tab menu: Close tab and Close other tabs.
- The View menu (`Alt+V`) on Start and Folders: Simple, Rich or Custom, side by side or unified, changed words and wrap. Your choice is saved in the profile.
- Pasted paths in double quotes (Explorer's "Copy as path") work on the Start screen.
- New compare tabs open with your saved look from the View menu (Simple, Rich or Custom).
- The Keys sheet lists the compare keys.
- The release zip carries the licences of the embedded fonts.

### Fixed

- In a narrow window, the hex view of one file no longer draws its text column over the right side. Each half stays inside its own column.
- The hex view fits its rows to the window: 16, 8 or 4 bytes per row, so no byte or text column is cut off below 1280 px.
- The View menu opens under the View button instead of at the bottom left of the window.
- After a compare opens or the window is resized, Folders shows size and date columns, and the top bar switches between its wide and compact forms, without waiting for a click or key press.
- The top bar names the open comparison (for example "left ↔ right") instead of always reading "New compare".

### Changed

- A click now selects a folder; double click or the arrow opens it.

### Known limits

- Some viewer keys (line/page scrolling, find results, undo) are not listed in the Keys sheet yet.
- The other limits of alpha.1 still apply, except the View menu and the narrow hex view, which are now fixed.

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
- No Sync plan, no copying of changes and no three-way merge.
- No Explorer menu and no auto-update.
