# Changelog

This file follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.1.0-alpha.4] — 2026-11-18

### Added

- Git compare: a branch, tag or commit against another one or a folder, read-only. Type `repo@revision` in a side field on the Start screen, or on the command line. Git compares appear in Recent. Sidewise never starts `git`, never runs programs from the repository, and never fetches.

### Known limits

- A working folder checked out with `core.autocrlf=true` (the default in Git for Windows) shows its text files as different from a revision, because the line endings differ.
- A file over 64 MiB in a revision shows as unreadable.
- The repository folder must be the top folder of the repository. A subfolder shows "Not a Git repository".
- A Git side must be a folder. A Git revision against a single file is refused.

### Fixed

- The right-click menu in the file diff and in the Stream now opens. In alpha.3 a right-click only picked lines. `Shift+F10` and the Menu key open the same menu next to the cursor line.
- A copy stopped before it changed the file now says "Rolled back an interrupted copy." at the next start. Alpha.3 rolled it back without a note.
- A Git revision that does not exist shows "No such revision" instead of an empty tab. `F5` compares a Git side again, so a branch shows its newest commit.

## [0.1.0-alpha.3] — 2026-11-17

### Added

- An opt-in update check. It is off until you press Turn on at the foot of the Start screen. On the third start, a card asks once. When on, Sidewise asks GitHub for the release list once a day. If a newer version exists, the status bar names it, with a link to the release page and a small x to hide it. GitHub sees your IP address and the client name `Sidewise`; Sidewise sends no other data. Failures are silent. Sidewise never downloads or installs anything. Turn off any time on the Start screen.
- Copy a change to the other side. `Ctrl+Alt+Right` copies the change at the cursor to the right, and `Ctrl+Alt+Left` copies it to the left. The right-click menu has the same commands, and also copies the lines you picked.
- Copy all matches in Stream: `Ctrl+Shift+Right` and `Ctrl+Shift+Left`. A sheet lists the files and what it skips, and you confirm with Copy all.
- Every copy is safe against a crash or a power cut. Sidewise saves the old file in a journal first, then replaces the file in one step. You see either the old file or the new one, never half of each.
- Undo for copies. The "Copied to the left" and "Copied to the right" message has an Undo button. `Ctrl+Z` undoes the latest copy in any open compare tab. Undo puts back the old bytes and the old modified time.
- Recovery at start. If Sidewise stopped during a copy, the next start puts the old file back and says so. A file you edited since then is kept as it is.
- Old backups are removed at start: after 14 days, and the oldest first when the journal passes 1 GiB.
- The README explains how to take a file back from the journal by hand.
- A Theme picker: five palettes (Violet, Graphite, Ocean, Ember and High contrast) and a System, Light or Dark mode.
- An installer, `sidewise-<version>-windows-x64-setup.exe`: it installs Sidewise for your Windows user only, with a Start menu entry and an uninstaller. It needs no administrator and keeps your settings when you uninstall. The portable zip stays.
- The release zip carries the licence of the embedded Material icons.

### Changed

- Sidewise is no longer read-only: it can copy changes between the two sides. Releases up to alpha.2 are read-only.
- Closing the last tab leaves a New compare tab open instead of quitting.
- Copy commands confirm with a short message.

### Fixed

- The Keys sheet lists all viewer keys (line and page scrolling, find results, undo).
- The View menu closes on a click outside it and when you switch tabs.
- Closing a menu with Esc returns focus to where it was.
- The Keys sheet names the find-result keys' section "Find results" instead of an internal name.
- Copying and Undo work on exFAT and FAT drives such as USB sticks. These drives have no stable file ids, so Sidewise re-reads and checks the file just before it replaces it, and refuses if it changed. A restored file keeps its old modified time on exFAT too.

### Removed

- The Git compare entry in the sidebar; Git compare is not in this alpha.

### Known limits

- Copy refuses a file that changed since it was shown, a read-only side, a file another program has open, and a file on a network drive. It copies nothing and tells you why.
- Undo works in the session that made the copy. It covers the latest copy first. After a restart, Sidewise does not offer Undo. The backups stay in the journal folder for 14 days; see the README to restore by hand.
- An undo is refused when the file changed after the copy. It writes nothing, and that copy cannot be undone later.
- If Sidewise cannot confirm that a copy was written, it says so and offers no Undo. It checks the file at the next start.
- If the journal cannot open, copying is off and Sidewise says why. Compare still works.
- Copying works on files only. Folder sync, merge and three-way merge are not built yet.
- FAT32 drives are not tested yet.
- Git compare is still not in this release. A Git side on the command line (`repo@rev`) exits with code 2.

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
