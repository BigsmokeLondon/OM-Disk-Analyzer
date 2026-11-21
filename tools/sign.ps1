# Sign OM Disk Analyzer with a code-signing certificate (run in PowerShell).
#
#   .\tools\sign.ps1 -Pfx C:\path\om-industries.pfx          # certificate file (asks for its password)
#   .\tools\sign.ps1 -Thumbprint 0123ABCD...                  # certificate installed in Windows (or a USB token)
#
# Signs every .exe in dist\ with SHA-256 and a free public timestamp, so the signature stays
# valid after the certificate expires. Needs signtool.exe from the Windows SDK.
param(
    [string]$Pfx,
    [string]$Thumbprint,
    [string]$Timestamp = "http://timestamp.digicert.com"
)
$ErrorActionPreference = "Stop"
$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signtool) { throw "signtool.exe not found: install the Windows SDK (Signing Tools for Desktop Apps)." }
$files = Get-ChildItem (Join-Path $PSScriptRoot "..\dist\*.exe")
if (-not $files) { throw "No .exe files in dist\ to sign." }
$args = @("sign", "/fd", "SHA256", "/tr", $Timestamp, "/td", "SHA256", "/d", "OM Disk Analyzer")
if ($Pfx) {
    $pw = Read-Host "Password for $Pfx" -AsSecureString
    $plain = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($pw))
    $args += @("/f", $Pfx, "/p", $plain)
} elseif ($Thumbprint) {
    $args += @("/sha1", $Thumbprint)
} else {
    throw "Give -Pfx <file> or -Thumbprint <certificate thumbprint>."
}
foreach ($f in $files) {
    & $signtool.FullName @args $f.FullName
    & $signtool.FullName verify /pa $f.FullName
}
