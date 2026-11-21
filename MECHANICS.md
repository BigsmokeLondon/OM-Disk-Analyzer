# OM Disk Analyzer — mechanics sheet

Rules read from the original program (TreeSize.exe, Delphi, 32-bit) with the built-in
decompiler (Ghidra isn't installed, so each value was checked against a second place:
the settings type table, the option forms or the person's picture). Addresses are the
original's virtual addresses.

## Sizes
| Rule | Value | Where in the original |
|---|---|---|
| Allocated space of a file | `ceil(size / cluster) × cluster`: `(size div c + 1) × c`, minus one cluster when `size mod c = 0`; 0-byte files take 0 | `sub_5eacc4` (per-file record), cluster in `edi` |
| Compressed or sparse files (attributes `0x800` / `0x200`) | `GetCompressedFileSizeW`, then rounded up to whole clusters; falls back to the plain size if the call fails | `sub_5eacc4` → `sub_6117c4` (wrapper around `GetCompressedFileSizeW`), checked again in `sub_5eab84` and `sub_615148` |
| Wasted space (slack) | `cluster − size mod cluster`, 0 when it divides evenly | `sub_5eacc4`, field `+0x28` |
| CD size | `ceil(size / 2048)` sectors | `sub_5eacc4`, field `+0x30`; type table `vt_CDSize` |
| Hard links | when tracking is on, size, allocated and wasted space are divided by the file's link count | `sub_5eacc4` → `sub_613608` (opens with `FILE_FLAG_BACKUP_SEMANTICS`, reads the link count) — *not built: off in the rebuild* |
| Long paths | paths get the `\\?\` prefix (`\\?\UNC\` for network paths) | `sub_6126e0`, used by `sub_611714` (`FindFirstFileW`) and `sub_6117c4` — the rebuild's Rust file calls add it themselves |

## Defaults (settings constructor `sub_810220`, published defaults in the type table at `0x80ef00`)
| Setting | Original default | Where |
|---|---|---|
| Tree shows | Allocated space (`ViewType = 3`, `vt_alloc`); the person's picture's title says "Allocated" too | `sub_810220`: `[esi+0xE0] = 3` |
| Size unit | MB (`SizeUnit = 2`, `Mega`) | `sub_810220`: `[esi+0xE1] = 2`; enum `TUnitType` |
| Sort | by size (`SortType = 1`, `stSize`) | `sub_810220`: `[esi+0xE3] = 1` |
| Decimals | 1 | type table `Decimals` default 1 |
| Bold folders | larger than **15** % of the scanned folder (`BoldFactor = 15`, `BoldFactorType = 0`); the other choice is Megabytes | `sub_810220`: `[esi+0xDC] = 15`; option form `BoldFactorTypeCombo` at `0x9586e4` |
| Size bar colours | `$0064FFFF` → `$000064FF`, i.e. **#FFFF64 → #FF6400** | `sub_810220`: `[esi+0xF8]`, `[esi+0xFC]` |
| Largest files listed | **100** (`NumberOfTopFiles`) | `sub_810220`: `[esi+0xB8] = 0x64` |
| Recent folders kept | 12 (`MRUMaximum`) | `sub_810220`: `[esi+0xA8] = 12` — the rebuild keeps 15 |
| Right pane refresh during a scan | 10,000 ms | `sub_810220`: `[esi+0xE8] = 0x2710` |
| Scan thread priority | `tpLower` on computers with more than one processor, else `tpNormal` | `sub_810220`: `[esi+0xF4]` |
| Follow links/junctions | off (`FollowReparsePoints = 0`) | type table |
| `[Files]` line | only in folders that also have subfolders (`AlwaysCreateFilesItem = 0`) | type table |
| Files in the tree | off (`DefaultShowFiles`), can be turned on | type table, View menu |
| Compressed folders in blue | on (`ColorForCompressed = 1`) | type table, form at `0x9580b1` |
| Drive list | shown (`ShowDrivesPanel = 1`) | type table |
| Long tooltips | on (`ShowLongTooltips = 1`) | type table |
| File age statistics | on, grouped by **Last Change Date** (`FileAgesDateType = 1`, `dtLastChange`) | type table; form at `0x95e345` |
| Treemap minimum box size | 5 px | type table `TreeMap2DMinimumSize` |

## Age of files
| Rule | Value | Where |
|---|---|---|
| Default intervals | **1, 7, 30, 182, 365, 730 days**, then "older" (`0x7FFFFFFF`) | `sub_816710` (reset method of `TFileAges`, class table `0x815d28` slot `+0x14`) |
| Interval list | sorted, duplicates dropped | `sub_816fb0` |
| Labels | Less than 1 day · 1 day – 1 week · 1 week – 1 month · 1 month – 6 months · 6 months – 1 year · 1 year – 2 years · More than 2 years | rebuild's own wording (the original's labels are resource strings) |
| A file's bucket | the first interval its age in days is below | *estimated* (whether the limit itself counts as inside was not settled) |

## Not taken from the code
* Bar shading inside one bar (left end at 40 % of the bar's share): *estimated* from the picture.
* Multi-threaded scanning (`MaximumThreads = 8`): the rebuild scans with one thread.
* Deleting from the tree (person's request, not in this sheet's original): only in administrator mode; Recycle Bin by default, or permanently.
