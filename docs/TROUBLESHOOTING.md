# Troubleshooting and FAQ

## Windows says "Windows protected your PC" or "Unknown publisher"
The program isn't code-signed yet. Click **More info > Run anyway**. Once OM Industries signs the
files with a code-signing certificate, the warning goes away (see [CODE_SIGNING.md](CODE_SIGNING.md)).

## Windows asks for permission every time I open it
Your account is an administrator, and OM Disk Analyzer starts with administrator rights so it
can scan protected folders and delete files. Choose **No** to run normally, or turn off
**Tools > Options > Appearance > Start as administrator when my account is an administrator**.

## Delete says it needs administrator mode
Deleting only works in administrator mode. Use **File > Restart as administrator** (or the button
in the Delete window) and say **Yes** to Windows' question. The title bar then says (Administrator).

## "Could not delete ...: it is still there (it may be in use)"
Another program has the file open. Close it (or restart the computer) and try again.

## Some folders are red or have a "!" on the icon
They couldn't be read, usually because Windows protects them (for example *System Volume
Information*). Run in administrator mode to read more of them. **Tools > Show scan errors**
lists every item that couldn't be read and why.

## The size of a drive doesn't match File Explorer exactly
* **Allocated** (the default) is the space files take on the disk, in whole clusters; **Size** is
  their real length. Switch with the **Size / Alloc** buttons.
* Folders the program can't read aren't counted.
* Links and junctions aren't followed by default (so nothing is counted twice); turn on
  *Follow symbolic links and junctions* in **Options > Scan** if you need them.
* Windows' own files (page file, hibernation file, restore points) may be hidden from all programs.

## Scanning is slow
Large drives with hundreds of thousands of files take a while. Turning off *owner statistics* in
**Options > Scan** (it's off by default) makes scans much faster. Network drives are slower than
local ones.

## The Users tab only shows a few owners
By default it checks the owners of the subfolders of the selected folder. For exact numbers
across every file, turn on *Collect owner statistics for every file* in **Options > Scan** and
scan again.

## Setup says it can't copy the program
OM Disk Analyzer is still open. Close every copy and run Setup again.

## How do I reset all settings?
Close the program and delete `%APPDATA%\OM Industries Inc\OM Disk Analyzer` (or the portable
copy's `Data` folder).

## Does it send anything over the internet?
No. See [PRIVACY.md](PRIVACY.md).

## Does it run on a Mac or Linux?
Version 2.0 is built and tested for Windows. The source is portable Rust, but Mac and Linux builds
aren't made or tested yet; version 1.2 (Python) has Mac builds.
