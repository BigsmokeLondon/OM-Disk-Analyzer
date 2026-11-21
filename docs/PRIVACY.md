# Privacy and security

OM Disk Analyzer works entirely on your computer.

* **Almost no internet.** The only connection it makes is one request when it starts (and when you
  choose *Help > Check for updates*) to GitHub's public release list, to learn the newest version
  number. Nothing about your computer, files or scans is sent. Turn it off in
  *Options > Appearance > Check for a newer version when the program starts*. There are no
  statistics, crash reports or advertising.
* **No account.** Nothing to sign in to.
* **Reads, doesn't change, while scanning.** A scan only reads folder listings and file
  details (names, sizes, dates, attributes and, if turned on, owners). File contents are never read.
* **Changes only what you ask for:**
  * files and folders you delete yourself (with a confirmation window),
  * the CSV files you export,
  * its own settings and scan history,
  * the File Explorer menu entry, only if you turn it on (in your user's part of the registry),
  * shortcuts and the Settings > Apps entry, only when installed with Setup.
* **What it stores:** settings (`settings-v3.json`) and a scan history (`history.json`: date,
  scanned folder path, total size, file and folder counts) in
  `%APPDATA%\OM Industries Inc\OM Disk Analyzer` (Windows) or
  `~/Library/Application Support/OM Industries Inc/OM Disk Analyzer` (Mac), or in the portable
  copy's `Data` folder.
  Delete them at any time, or tick *Also remove my settings and scan history* when uninstalling.

## Administrator rights
For administrator accounts the program asks Windows to start it with administrator rights, so it
can read protected folders and delete protected files. Windows asks you each time, and you can say
No or turn the behaviour off in Options. Take care when deleting in administrator mode: Windows
won't stop you from deleting system or program files.

## Reporting a problem
Contact OM Industries Inc with the version number (Help > About) and what you were doing.
