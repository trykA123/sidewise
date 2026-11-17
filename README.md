# Sidewise

Sidewise compares two folders or two files. It is built from its own crates in this repository. They began as the compare engine of [Navo](https://github.com/trykA123/navo) and were imported with their history.

Status: alpha pre-release 0.1.0-alpha.2. Windows only. Read-only: it never changes your files.

See the [CHANGELOG](CHANGELOG.md) for the full list of what this release has and lacks.

## Download

Get the files from the [Releases page](https://github.com/trykA123/sidewise/releases):

- `sidewise-0.1.0-alpha.2-windows-x64.zip`
- `SHA256SUMS`

Check the zip before you run it:

1. Open PowerShell in the download folder.
2. Run `Get-FileHash -Algorithm SHA256 .\sidewise-0.1.0-alpha.2-windows-x64.zip`.
3. Check that the `Hash` value matches the line for the zip in `SHA256SUMS`.

Unzip the file into any folder. Then start `sidewise.exe`.

The build is not signed. Windows SmartScreen warns on the first start. To run it anyway:

1. Click **More info**.
2. Click **Run anyway**.

## Use

Start `sidewise.exe` and pick two folders or two files on the Start screen.

Or run `sidewise <left> <right>` in a terminal. Run `sidewise --help` for the other flags.

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
- An installer, an Explorer menu and auto-update.

## Layout

- `crates/sidewise-*`: the compare engine, file layer, host glue and compare screen.
- `crates/navo-compare-app`: the app (the `sidewise` binary).
- `crates/navo-compare-cli`: the command-line parser.
- `crates/navo-compare-profile`: the profile, recent and named sessions.
- `assets/brand`: the app icon.
- `scripts/`: the local gate and the Windows build.

## Build

```
cargo build -p navo-compare-app
```

## Licence

MIT or Apache-2.0, at your choice. See `LICENSE-MIT` and `LICENSE-APACHE`.
