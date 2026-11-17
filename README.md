# Sidewise

Sidewise compares two folders or two files. It is built from its own crates in this repository. They began as the compare engine of [Navo](https://github.com/trykA123/navo) and were imported with their history.

Status: alpha pre-release 0.1.0-alpha.3. Windows only. It can copy changes between the two sides, with Undo, and it never leaves a half-written file. See [Copy and Undo](#copy-and-undo).

See the [CHANGELOG](CHANGELOG.md) for the full list of what this release has and lacks.

## Download

Get the files from the [Releases page](https://github.com/trykA123/sidewise/releases):

- `sidewise-0.1.0-alpha.3-windows-x64-setup.exe`
- `sidewise-0.1.0-alpha.3-windows-x64.zip`
- `SHA256SUMS`

Check each file before you run it:

1. Open PowerShell in the download folder.
2. Run `Get-FileHash -Algorithm SHA256 .\sidewise-0.1.0-alpha.3-windows-x64-setup.exe`. Do the same for `sidewise-0.1.0-alpha.3-windows-x64.zip`.
3. Check that the `Hash` value matches the line for that file in `SHA256SUMS`.

### Install

Run the setup. It installs Sidewise for your Windows user only, in `%LOCALAPPDATA%\Programs\Sidewise`, and needs no administrator. Start Sidewise from the Start menu.

### Portable

Unzip the file into any folder. Then start `sidewise.exe`.

The setup and `sidewise.exe` are not signed. Windows SmartScreen may warn on the first start. To run it anyway:

1. Click **More info**.
2. Click **Run anyway**.

## Use

Start `sidewise.exe` and pick two folders or two files on the Start screen.

Or run `sidewise <left> <right>` in a terminal. Run `sidewise --help` for the other flags.

## Update check

Sidewise can tell you when a new version is out. This is off until you turn it on. Until then, Sidewise makes no network call.

- The foot of the Start screen has a line: "Sidewise does not check for new versions." Press **Turn on** to enable the check.
- On the third start, if you have not chosen, the line becomes a small card: "Get a note when a new Sidewise is out?" Press **Turn on** or **Not now**. Not now means off, and the card does not come back.
- When the check is on, Sidewise asks GitHub for the public release list of `trykA123/sidewise`. It asks at most once a day, a few seconds after start. Turning it on runs the first check at once.
- GitHub sees your IP address, the client name `Sidewise` and the request for the release list. Sidewise sends no version, system, account or file data. The request uses your Windows proxy settings.
- If a newer version exists, the status bar shows a dot and "Sidewise 0.1.0-alpha.3 is available." (the number is the new version). Click the text to open the release page in your browser. Click the small x to hide the note until a still newer version appears.
- The Start line says the same and has **Release page** and **Turn off**. Tab reaches them; Enter or Space presses them.
- If you are offline or GitHub does not answer, Sidewise shows nothing and tries again the next day.
- Sidewise never downloads or installs anything. You get the new version from the release page.

To turn it off, press **Turn off** on the Start screen. The next start makes no network call. Sidewise keeps the time of the last check and the newest version it found in `update-check.toml` in `%LOCALAPPDATA%\Sidewise`. Your choice is the `update_check` value in `settings.toml` in `%APPDATA%\Sidewise`.

## Copy and Undo

Sidewise copies changes between the two sides. Releases up to alpha.2 do not.

- Copy the change at the cursor: `Ctrl+Alt+Right` to the right, `Ctrl+Alt+Left` to the left.
- Copy picked lines or one change: use the right-click menu.
- Copy all matches in Stream: `Ctrl+Shift+Right` or `Ctrl+Shift+Left`. A sheet lists the files. Choose Copy all to confirm.

A copy writes at once. There is no Save step. Sidewise saves the old file in the journal first. Then it replaces the file in one step. After a crash or power cut, the file is the old one or the new one, never half of each.

A short message tells you the result, for example "Copied to the right". It has an Undo button. `Ctrl+Z` undoes the latest copy. Undo puts back the old bytes and the old modified time.

Sidewise refuses a copy and writes nothing in these cases:

- The file changed after Sidewise showed it: "Changed since shown. Nothing was copied."
- The side is read-only: "This side is read-only. Nothing was copied."
- The file is on a network drive: "Sidewise does not write to network drives. Nothing was copied."
- Another program has the file open: "Another program is using this file. Close it there, then try again. Nothing was copied."

Undo refuses when the file changed after the copy: "The file changed after the copy. Nothing was undone." That copy cannot be undone later.

If Sidewise cannot confirm that a copy was written, it says: "The change may have been written, but Sidewise could not confirm it. It cannot be undone now; Sidewise checks the file at the next start."

If the journal cannot open, Sidewise says "Copying is off" and gives the reason. Compare still works.

### After a crash

If Sidewise stops during a copy, the next start puts the old file back. It tells you how many files it restored. A file that you changed since then stays as it is, and the message says so. If a file cannot be restored now, Sidewise tries again at the next start. When Sidewise leaves a file as it is, the message also names the journal folder.

### Taking a file back from the journal

Undo works only in the session that made the copy. After a restart, there is no Undo button. The old files stay in the journal folder. Use them to take a file back by hand.

Sidewise keeps a backup for 14 days. At each start it removes backups older than 14 days. Then it removes the oldest ones until all backups fit in 1 GiB. A removed backup cannot come back. Backups of the current session stay until the next start.

To restore a file by hand:

1. Close Sidewise. This stops it from touching the journal while you work.
2. Open the folder `%LOCALAPPDATA%\Sidewise\journal\v1`. Paste that path into the Explorer address bar.
3. Find the job folder. Its name is `<time>-<process>-<number>`. The time is in milliseconds since 1 January 1970, so a higher number is newer. For example, `1763380000000-4120-0`.
4. Open the job folder. It holds a file named `log` and one or more files named `0.bak`, `1.bak` and so on. Each `<n>.bak` is the old content of one file that the job changed.
5. Open `log` in a text editor. Find the line that starts with `{"t":"Intent","item":<n>`. The `"target"` value is the path of the file that the copy changed. In the log a backslash is written twice, and a non-ASCII letter as `\uXXXX`.
6. Copy `<n>.bak` over that file. Keep the original name and extension of the target, not `.bak`. Copy it first to a safe place if you are not sure.
7. Start Sidewise again. Compare the two sides to check the result.

The modified time of the restored file is the time of your copy, not the old time.

Do not edit or delete files in the journal folder while Sidewise runs.

## Uninstall

Open Settings > Apps > Installed apps, find Sidewise and choose Uninstall. Your settings, sessions and backups stay in `%APPDATA%\Sidewise` and `%LOCALAPPDATA%\Sidewise`. Delete those two folders to remove them too. The journal folder with your backups is inside `%LOCALAPPDATA%\Sidewise`. For the zip: delete its folder.

## In this alpha

- Compare two folders or two files, in tabs.
- Folders view with filters and a change map.
- File diff for text, hex and images.
- Stream view with an Outline.
- Recent sessions on the Start screen.
- Five colour themes, with System, Light and Dark mode.
- Copy a change to the other side, with Undo.
- Command line: `sidewise <left> <right>`, `--profile DIR`, `--version` and `--help`.

## Not in this alpha

- Git branch or ref compare.
- Accounts.
- Merging, syncing folders and three-way merge.
- An Explorer menu, auto-update (the optional update check only names a new version) and a signed build.

## Layout

- `crates/sidewise-*`: the compare engine, file layer, host glue and compare screen.
- `crates/navo-compare-app`: the app (the `sidewise` binary).
- `crates/navo-compare-cli`: the command-line parser.
- `crates/navo-compare-profile`: the profile, recent and named sessions.
- `crates/sidewise-update`: the update check: version rules, the GitHub release list and the request.
- `assets/brand`: the app icon.
- `installer/`: the Inno Setup script of the installer.
- `scripts/`: the local gate, the Windows build and the release.

## Build

```
cargo build -p navo-compare-app
```

## Licence

MIT or Apache-2.0, at your choice. See `LICENSE-MIT` and `LICENSE-APACHE`.
