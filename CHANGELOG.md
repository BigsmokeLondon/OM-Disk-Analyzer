# Changelog

## 2.0 — November 2026
A complete rewrite in Rust: one fast, self-contained program with no runtime to install.

**New**
* Deleting files and folders from the tree, Details and Top 100 Files (Recycle Bin or
  permanently), in administrator mode.
* Administrator accounts start the program with administrator rights automatically (can be
  turned off in Options); *File > Restart as administrator*.
* *View > Show files in tree* lists single files under their folders.
* Branded install wizard and uninstaller built into the program; portable copy with its own
  `Data` folder.
* **macOS version** (Apple Silicon and Intel `.dmg`): the same program, with the Trash instead of
  the Recycle Bin and Finder instead of Explorer. Built automatically on GitHub; see the
  [Mac tester guide](docs/MAC_TESTER_GUIDE.md).
* **Update check:** tells you at start-up (and under *Help > Check for updates*) when a newer
  version is published. It can be turned off in Options.
* Automatic releases: pushing a version tag builds Windows and Mac files on GitHub.
* Packaging and code-signing scripts (`tools/package.ps1`, `tools/sign.ps1`).
* Full documentation in `docs/`.

**Changed (defaults now follow the reference program's own code; see `MECHANICS.md`)**
* The tree shows **allocated space** by default, in MB, sorted by size.
* Size bars run from #FFFF64 to #FF6400; folder names are bold at 15 % of the scanned folder.
* Age of Files groups by 1 day, 1 week, 1 month, 6 months, 1 year and 2 years.
* Compressed and sparse files count their real space on disk.
* The *[Files]* line only appears in folders that also have subfolders.
* Settings are kept in a new file (`settings-v3.json`); the scan history from 1.2 is kept.

**Not in 2.0 yet**
* Linux builds.
* Counting hard-linked files once, and scanning with several threads.

## 1.2 — November 2026
Python/tkinter version with folder tree, drive list, charts, treemap, file types, owners, age of
files, largest files, scan history, search, CSV export, Options, File Explorer menu, Windows
Setup, portable copy and Mac builds. Source in `python-version/`.
