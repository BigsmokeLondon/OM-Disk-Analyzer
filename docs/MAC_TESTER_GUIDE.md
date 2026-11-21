# Mac tester guide

Thank you for trying OM Disk Analyzer on a Mac. It takes about 15 minutes. You need a Mac running
macOS 11 or later.

## 1. Install
1. Download the file for your chip: **Apple Silicon** (M1 or later) or **Intel**. (Apple menu >
   About This Mac shows which you have.)
2. Open the `.dmg` and drag **OM Disk Analyzer** onto **Applications**.
3. Right-click the app in Applications > **Open** > **Open**. (Needed only the first time.)

Write down: macOS version, chip, and anything macOS said or asked.

## 2. Things to try
Tick each one and note anything odd:
- [ ] The window opens with the OM logo, and the About box shows the right version.
- [ ] The drive list (bottom left) shows your Mac's disk and its free space, and any external disks.
- [ ] Double-click the main disk (or type `/Users` at the top and press Enter) and scan.
- [ ] The folder tree fills with sizes and bars; the biggest folders are on top.
- [ ] The tabs work: Chart, Treemap, Extensions, Users (owners), Age of Files, Top 100 Files, History.
- [ ] *Reveal in Finder* (right-click a folder) opens Finder with the item selected.
- [ ] Right-click a **test file you don't mind losing** > Delete > *Move to Trash*. Check it is in the Trash.
- [ ] *File > Export to CSV* makes a file that opens in Numbers or Excel.
- [ ] ⌘ shortcuts work, and the menus and tooltips say ⌘ (for example ⌘E for Export).
- [ ] Quit and reopen: your settings and the History tab are still there.
- [ ] *Help > Check for updates...* shows a message and doesn't freeze the window.
- [ ] Without Full Disk Access, scanning `/Users/<you>` lists some folders under *Show scan errors*.
      After giving Full Disk Access (see INSTALLING.md) and restarting, those errors go away.

## 3. What to send back
- macOS version and chip, and which file you installed
- The step that went wrong and what you saw (a screenshot helps)
- Whether sizes look right: compare one folder with Finder's *Get Info* (Finder shows 1 GB as
  1,000 MB and the program shows 1,024 MB, so small differences of that kind are normal)

Settings and scan history are kept in `~/Library/Application Support/OM Industries Inc/OM Disk Analyzer`.
Deleting that folder resets the program.
