# Developer guide

OM Disk Analyzer 2.0 is a single Rust program using [eframe/egui](https://github.com/emilk/egui)
0.37 for the window. The same executable is the app, its installer and its uninstaller.

## Requirements
* Windows 10/11, 64-bit
* Rust (stable) from <https://rustup.rs> — built with rustc 1.100
* PowerShell 5+ for packaging; the Windows SDK's `signtool.exe` only for signing

## Building and running
    cargo build                    # debug build: target\debug\om-disk-analyzer.exe
    cargo build --release          # release build: target\release\om-disk-analyzer.exe
    cargo run -- "D:\Photos"       # open and scan a folder
    cargo test                     # unit tests

Release builds use the Windows GUI subsystem (no console window); debug builds keep the console
for messages.

## macOS
The same code builds on macOS. `src/sys.rs` has the Mac versions of the operating-system calls:
drives come from `/` and `/Volumes`, free space from `df -Pk`, the block size stands in for the
cluster size, owners come from `id -un` (cached), *Delete* moves into `~/.Trash` (Finder through
`osascript` for another disk), and *Reveal* is `open -R`. Settings live in
`~/Library/Application Support/OM Industries Inc/OM Disk Analyzer`.
`tools/build_mac.sh [arm64|x86_64]` builds the `.app` (Info.plist, `om.icns`, ad-hoc signature)
and the drag-to-Applications `.dmg` into `dist/`. Windows-only menu items are hidden with
`cfg!(windows)`; names such as "Recycle Bin"/"Trash" come from constants in `sys.rs`.
Apple notarization needs an Apple Developer account and isn't set up (see
[CODE_SIGNING.md](CODE_SIGNING.md)).

## Releases
`.github/workflows/build.yml` runs the tests and builds Windows (`tools/package.ps1`) and both Mac
chips on GitHub's computers. Run it by hand from the Actions tab, or push a tag
(`git tag v2.0.1 && git push origin v2.0.1`) to also create the GitHub Release with all files.
`tools/Link_To_GitHub.ps1` creates the **public** `OM-Disk-Analyzer` repository the first time
(it asks you to confirm first, because everything pushed becomes readable by anyone).

## Update check
`src/update.rs` asks `https://api.github.com/repos/BigsmokeLondon/OM-Disk-Analyzer/releases/latest`
through the `curl` program (built into Windows 10/11 and macOS) on a background thread, compares the
tag with `Cargo.toml`'s version and shows a window if it is newer. It reads only the tag name.
GitHub answers **404 for a private repository**, so the repository has to be public for this to work.
The owner and repository names are constants at the top of the file.

## Packaging
    .\tools\package.ps1 [-Pfx cert.pfx | -Thumbprint <sha1>]

Builds the release program and writes to `dist\`:
`<v> Setup.exe` (a copy of the program with "Setup" in its name), `<v> Portable.zip` (program,
`Data\`, read-me) and `<v> - source code.zip`. A plain `<v>.exe` is built along the way (it is what
gets signed and goes into the Portable zip) and is removed at the end, so it is not a release file.
The version comes from `Cargo.toml`.

## Command line
| Option | Does |
|---|---|
| `[folder]` | Scan this folder at start (Explorer's menu passes `C:"` for drives; repaired to `C:\`) |
| `--setup` | Install wizard (also when the file name contains "Setup") |
| `--uninstall` | Remove an installed copy |
| `--screenshot <file.png>` | Start without focus (behind other windows on Windows), scan a sample folder if none is given, draw at least 120 frames, save the window as PNG and exit. Never asks for administrator rights and never saves settings or history. |
| `--tab <name>` | Start on a tab (`chart`, `details`, `extensions`, `users`, `age`, `top`, `history`, `treemap`) |
| `--about`, `--options` | Open those windows (test pictures) |
| `--show-files` | Show single files in the tree for this run, and open the biggest folders (test pictures) |
| `--elevated` | Internal: marks the copy started with administrator rights so it doesn't ask again |

## Source layout
| File | Purpose |
|---|---|
| `src/main.rs` | Command line, start-up, automatic elevation, window options |
| `src/app.rs` | Main window state; menus, toolbar, info bar, status bar, folder tree, drive list; scanning hand-off; delete logic and tree updates |
| `src/app/tabs.rs` | Shared list widget (`table`), the eight tabs, the About / Options / Search / Errors windows |
| `src/scan.rs` | Background scanner: builds a flat `Vec<Node>` (children after parents, so one backward pass sums totals), extension / owner / age statistics, top-100 heap, file search |
| `src/sys.rs` | Windows calls: drives, cluster size, owners, compressed size, elevation, Recycle Bin delete, restart as administrator, File Explorer menu (via `reg.exe`) |
| `src/setup.rs` | Install wizard and uninstaller (per user; shortcuts through PowerShell's `WScript.Shell`, uninstall key through `reg.exe`) |
| `src/settings.rs` | Saved settings (`settings-v3.json`) and scan history (`history.json`, same format as 1.2) |
| `src/fmt.rs` | Number, size, date and wildcard helpers, file-type names |
| `src/draw.rs` | Brand colours, icons, gradient bars, ring slices, squarified treemap |
| `build.rs` | Embeds the icon and version information (winresource) |
| `assets/` | OM Industries logos (icon, badge, horizontal) and `om.ico` |
| `tools/package.ps1`, `tools/sign.ps1` | Packaging and signing |
| `MECHANICS.md`, `mechanics.json` | Rules and defaults taken from the reference program's code, with addresses |
| `python-version/` | The earlier 1.2 version (Python/tkinter), kept for reference |

## How a scan works
1. `scan::start` spawns a thread that runs the scan and returns a shared `Progress` (atomic
   counters, current folder, stop flag, result slot). The scan runs on 1 to 8 worker threads
   (Options > Scan > *Scanning threads*, default automatic: the number of cores, at most 8; one
   when *follow links* is on, so the same folder always wins when two paths lead to it). Each
   worker has the Options priority.
2. Workers take folders from a shared queue. A worker reads one folder with `std::fs::read_dir`
   without holding any lock (directory-entry metadata gives size, times and attributes without
   opening files), then briefly locks the shared node list to add the result and queue the
   sub-folders. Reparse points are skipped unless *follow links* is on; followed links are
   de-duplicated by canonical path.
3. Each file adds to its folder's own totals and to the worker's own statistics (extensions,
   owners, ages, largest files), added together at the end. Allocated space is
   `ceil(size / cluster) × cluster`; compressed/sparse files use `GetCompressedFileSizeW`.
4. *Count hard links once* (`sys::hard_link_key`: device + inode on a Mac; on Windows the file
   index, which needs opening each file, so it starts off there): files with several names are
   held back, sorted by path, and counted once under the first name. Folders' children are then
   sorted by name. Together this makes the result the same for any thread count.
5. One backward pass over the node list adds each folder into its parent.
6. The window polls `Progress` every 100 ms and swaps in the finished tree (`Arc<Vec<Node>>`).

Files themselves aren't kept in the tree (to keep memory low for whole drives); the Details
list and *Show files in tree* read a folder's files when it is opened and cache them.

## Deleting
`sys::delete_path` uses `SHFileOperationW` with `FOF_ALLOWUNDO` for the Recycle Bin, or
`remove_dir_all` / `remove_file` when permanent. After a delete, `App::apply_delete` subtracts
the item from every ancestor and removes it from its parent, so no rescan is needed.
Deleting is only offered when `sys::is_admin()` is true.

## Administrator start-up
`sys::can_elevate()` checks the process token for `TokenElevationTypeLimited` (an administrator
running without its rights). If so, and the setting is on, `main` restarts the program through
`ShellExecuteW("runas")` with the same arguments plus `--elevated`, and exits. If the person
says No, it carries on normally. A runtime restart is used instead of a `highestAvailable`
manifest so that Setup, uninstall and automated test pictures never trigger a prompt.

## Tests
`cargo test` covers number formatting and wildcards, age buckets and allocation rounding,
permanent delete of a temporary folder, command-line quoting for the elevated restart, and
shortcut creation.
