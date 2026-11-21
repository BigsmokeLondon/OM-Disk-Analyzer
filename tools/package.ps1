# Build the finished files for OM Disk Analyzer into dist\ (run in PowerShell from the app folder):
#
#   .\tools\package.ps1                       # build and package
#   .\tools\package.ps1 -Pfx cert.pfx         # ...and sign the programs first (see tools\sign.ps1)
#
# Makes:
#   OM Disk Analyzer <v> Setup.exe           installer (current user, no administrator rights)
#   OM Disk Analyzer <v> Portable.zip        unzip anywhere; settings stay in its Data folder
#   OM Disk Analyzer <v> - source code.zip   source code and logos
param([string]$Pfx, [string]$Thumbprint)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$v = (Select-String -Path Cargo.toml -Pattern '^version = "(\d+)\.(\d+)\.(\d+)"').Matches[0].Groups
$version = "$($v[1].Value).$($v[2].Value)"
if ($v[3].Value -ne "0") { $version += ".$($v[3].Value)" }
$name = "OM Disk Analyzer $version"

Write-Host "Building $name..."
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
$exe = Join-Path $root "target\release\om-disk-analyzer.exe"

$dist = Join-Path $root "dist"
New-Item -ItemType Directory -Force $dist | Out-Null
$stage = Join-Path $root "target\package"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null

# the same program twice: a file name containing "Setup" opens the install wizard.
# The plain copy is only used to fill the Portable zip (and to be signed); it is removed below.
Copy-Item $exe (Join-Path $dist "$name.exe") -Force
Copy-Item $exe (Join-Path $dist "$name Setup.exe") -Force

if ($Pfx -or $Thumbprint) {
    $signArgs = @{}
    if ($Pfx) { $signArgs.Pfx = $Pfx } else { $signArgs.Thumbprint = $Thumbprint }
    & (Join-Path $PSScriptRoot "sign.ps1") @signArgs
}

# portable: the program, a Data folder (settings and history stay there) and a short read-me
$port = Join-Path $stage "$name Portable"
New-Item -ItemType Directory -Force (Join-Path $port "Data") | Out-Null
Copy-Item (Join-Path $dist "$name.exe") (Join-Path $port "OM Disk Analyzer.exe")
Set-Content -Encoding UTF8 (Join-Path $port "Data\About this folder.txt") @"
OM Disk Analyzer keeps its settings and scan history in this folder while it is here,
so nothing is written to the computer it runs on. Keep it beside OM Disk Analyzer.exe.
"@
Set-Content -Encoding UTF8 (Join-Path $port "Read me.txt") @"
$name Portable - OM Industries Inc

Double-click "OM Disk Analyzer.exe" to start. No installation needed; it also runs from a USB stick.
Settings and scan history stay in the Data folder next to it.
To remove it, close it and delete this folder.
The User Guide and other documents are in the Documents folder.

To delete files or folders from the tree, run it as administrator
(File > Restart as administrator).
"@
# the documents, readable in any browser or text editor
Copy-Item -Recurse (Join-Path $root "docs") (Join-Path $port "Documents")
Copy-Item (Join-Path $root "LICENSE.md") (Join-Path $port "Documents\LICENSE.md")
Copy-Item (Join-Path $root "CHANGELOG.md") (Join-Path $port "Documents\CHANGELOG.md")
$zip = Join-Path $dist "$name Portable.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $port -DestinationPath $zip
Remove-Item -Force (Join-Path $dist "$name.exe")   # not a release file: Setup.exe and the Portable zip cover it

# source code
$src = Join-Path $stage "$name - source code"
New-Item -ItemType Directory -Force $src | Out-Null
foreach ($item in "src", "assets", "tools", "docs", "Cargo.toml", "Cargo.lock", "build.rs", "README.md", "CHANGELOG.md", "LICENSE.md", "MECHANICS.md", "mechanics.json") {
    Copy-Item -Recurse (Join-Path $root $item) $src
}
$szip = Join-Path $dist "$name - source code.zip"
if (Test-Path $szip) { Remove-Item -Force $szip }
Compress-Archive -Path $src -DestinationPath $szip

Remove-Item -Recurse -Force $stage
Get-ChildItem $dist -Filter "$name*" | Format-Table Name, @{n = "Size (MB)"; e = { [math]::Round($_.Length / 1MB, 1) } } -AutoSize
