# Sidewise

Sidewise compares two folders or two files. It is built from its own crates in this repository. They began as the compare engine of [Navo](https://github.com/trykA123/navo) and were imported with their history.

Status: alpha pre-release 0.1.0-alpha.2. Windows only. Read-only: it never changes your files.

See the [CHANGELOG](CHANGELOG.md) for the full list of what this release has and lacks.

## Download

Get the files from the [Releases page](https://github.com/trykA123/sidewise/releases):

- `sidewise-0.1.0-alpha.2-windows-x64-setup.exe`
- `sidewise-0.1.0-alpha.2-windows-x64.zip`
- `SHA256SUMS`

Check the file before you run it:

1. Open PowerShell in the download folder.
2. Run `Get-FileHash -Algorithm SHA256 .\<file>`.
3. Check that the `Hash` value matches the line for that file in `SHA256SUMS`.

### Install

Run the setup. It installs Sidewise for your Windows user only, in `%LOCALAPPDATA%\Programs\Sidewise`, and needs no administrator. Start Sidewise from the Start menu.

### Portable

Unzip the file into any folder. Then start `sidewise.exe`.

The files are not signed. Windows SmartScreen warns on the first start of the setup or of `sidewise.exe`. To run it anyway:

1. Click **More info**.
2. Click **Run anyway**.

## Use

Start `sidewise.exe` and pick two folders or two files on the Start screen.

Or run `sidewise <left> <right>` in a terminal. Run `sidewise --help` for the other flags.

## Uninstall

Open Settings > Apps > Installed apps, find Sidewise and choose Uninstall. Your settings, sessions and backups stay in `%APPDATA%\Sidewise` and `%LOCALAPPDATA%\Sidewise`. Delete those two folders to remove them too. For the zip: Delete its folder.

## In this alpha

- Compare two folders or two files, in tabs.
- Folders view with filters and a change map.
- File diff for text, hex and images.
- Stream view with an Outline.
- Recent sessions on the Start screen.
- Light and Dark mode, with the Violet colour scheme.
- Command line: `sidewise <left> <right>`, `--profile DIR`, `--version` and `--help`.

## Not in this alpha

- Git branch or ref compare.
- Accounts and theme switching.
- Copying changes, merging, syncing and three-way merge.
- An Explorer menu, auto-update and a signed build.

## Layout

- `crates/sidewise-*`: the compare engine, file layer, host glue and compare screen.
- `crates/navo-compare-app`: the app (the `sidewise` binary).
- `crates/navo-compare-cli`: the command-line parser.
- `crates/navo-compare-profile`: the profile, recent and named sessions.
- `assets/brand`: the app icon.
- `installer/`: the Inno Setup script of the installer.
- `scripts/`: the local gate, the Windows build and the release.

## Build

```
cargo build -p navo-compare-app
```

## Licence

MIT or Apache-2.0, at your choice. See `LICENSE-MIT` and `LICENSE-APACHE`.
