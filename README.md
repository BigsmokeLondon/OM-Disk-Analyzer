# OM Disk Analyzer Professional — OM Industries Inc

A disk space analyzer for Windows: folder tree with size bars, drive list, charts, treemap,
file types, owners, age of files, largest files, scan history, search, CSV export, Options
and an optional File Explorer right-click menu.

## The finished files (in `dist/`)
| File | Use it for |
|---|---|
| `OM Disk Analyzer 1.2 Setup.exe` | Windows installer: Start menu entry (and optional desktop shortcut), uninstaller in Start menu and Settings > Apps. Installs for your user only, no admin rights. |
| `OM Disk Analyzer 1.2 Portable.zip` | Portable: unzip anywhere (USB stick too); settings stay in its `Data` folder; delete the folder to remove. |
| `OM Disk Analyzer.exe` | The program on its own. |
| `OM Disk Analyzer 1.2 - source code.zip` | All source code and logos. |

### macOS (Intel and Apple Silicon)
Mac programs must be built on a Mac. The easiest way from Windows: double-click
**`Build Mac versions.cmd`**. It uploads the app to a private repository in your GitHub account, lets GitHub's
Macs build both installers and puts `OM Disk Analyzer 1.2 macOS Intel.dmg` and
`OM Disk Analyzer 1.2 macOS Apple Silicon.dmg` into `dist/` (about 10 minutes). Other ways:
* **GitHub (no Mac needed):** put this folder in a GitHub repository, open *Actions > Build OM Disk Analyzer >
  Run workflow*. It builds `OM Disk Analyzer 1.2 macOS Intel.dmg` and `... macOS Apple Silicon.dmg`
  (and the Windows files) and offers them under *Artifacts*.
* **On a Mac:** install Python 3.12+ from python.org, then run `bash tools/build_mac.sh arm64` or
  `bash tools/build_mac.sh x86_64`.

The Mac app is not signed with an Apple Developer ID, so the first time, right-click it and choose *Open*.
On a Mac the File Explorer menu option is not available; everything else works the same.

## Run from the source code
The program is written in Python 3 (3.12 or newer) using only its standard library (tkinter).

    python main.py              # open the window
    python main.py "D:\Photos"  # open and scan a folder

## Files
| File | What it holds |
|---|---|
| `main.py` | Starts the program |
| `omdisk.py` | Scanner, size-bar folder tree, colours, logo loading, screenshots |
| `omdisk_app.py` | Main window: toolbar, tabs, charts, history, search, export |
| `omdisk_settings.py` | Options window, saved settings, File Explorer menu |
| `assets/` | OM Industries logos at ready-made sizes |
| `tools/make_assets.py` | Rebuilds `assets/` from the logo pack (needs Pillow) |
| `om_install.py`, `installer.py` | Windows setup program and uninstaller |
| `tools/build_all.py` | Builds all Windows files above (needs PyInstaller) |
| `tools/build_mac.sh`, `.github/workflows/build.yml` | Mac build script and the GitHub build |

Settings and scan history are kept in `%APPDATA%\OM Industries Inc\OM Disk Analyzer` on Windows,
`~/Library/Application Support/OM Industries Inc/OM Disk Analyzer` on a Mac, or in the `Data`
folder of the portable copy.

## Rebuild the Windows files
    pip install pyinstaller
    python tools/build_all.py
