# Link_To_GitHub.ps1 - OM Disk Analyzer (OM Industries Inc)
#
# Puts this folder in a PUBLIC GitHub repository called OM-Disk-Analyzer and pushes it, without
# changing any of your files. Public, so that anyone can download releases and the program's
# "Check for updates" can read the release list. Safe to run more than once. Run it in PowerShell
# from the app folder:
#
#     .\tools\Link_To_GitHub.ps1
#
# Afterwards, to publish a release (this builds Windows + Mac and attaches every file):
#     git tag v2.0.0
#     git push origin v2.0.0
$ErrorActionPreference = 'Continue'
$repo = 'OM-Disk-Analyzer'
$app = Split-Path -Parent $PSScriptRoot
Set-Location $app

function Say($t) { Write-Host ""; Write-Host "== $t" -ForegroundColor Cyan }

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Say "Git is not installed"
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        winget install --id Git.Git -e --source winget --accept-package-agreements --accept-source-agreements
        $env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
    }
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw 'Install Git from https://git-scm.com/download/win and run this again.' }
}
$gh = (Get-Command gh -ErrorAction SilentlyContinue).Source
if (-not $gh) { $gh = Join-Path $env:USERPROFILE 'gh\bin\gh.exe' }
if (-not (Test-Path $gh)) { throw 'The GitHub CLI is needed: install it from https://cli.github.com and run this again.' }
& $gh auth status *> $null
if ($LASTEXITCODE -ne 0) { Say "Sign in to GitHub"; & $gh auth login --web --scopes workflow }
$owner = (& $gh api user --jq .login).Trim()

Say "This repository will be PUBLIC"
Write-Host "Everyone on the internet can read everything in this folder that gets pushed: the source"
Write-Host "code, MECHANICS.md and mechanics.json (rules taken from the reference program), the docs"
Write-Host "and the screenshots. Anything listed in .gitignore is left out."
$ok = Read-Host "Type YES to continue"
if ($ok -ne 'YES') { Write-Host "Cancelled. Nothing was uploaded."; exit 0 }

Say "Preparing the source code"
if (-not (Test-Path .git)) {
    git init -q -b main
    git config user.name "OM Industries Inc"
    git config user.email "$owner@users.noreply.github.com"
}
git add -A
git diff --cached --quiet
if ($LASTEXITCODE -ne 0) { git commit -q -m "Update OM Disk Analyzer" }

Say "Uploading to the public repository $owner/$repo"
& $gh repo view "$owner/$repo" *> $null
if ($LASTEXITCODE -ne 0) {
    & $gh repo create $repo --public --description "OM Disk Analyzer - OM Industries Inc" --source . --remote origin --push
} else {
    git remote get-url origin *> $null
    if ($LASTEXITCODE -ne 0) { git remote add origin "https://github.com/$owner/$repo.git" }
    git push -u origin main
}
if ($LASTEXITCODE -ne 0) { throw "The upload did not work." }
# a repository that already existed as private is switched to public
$vis = (& $gh repo view "$owner/$repo" --json visibility --jq .visibility).Trim()
if ($vis -ne 'PUBLIC') {
    & $gh repo edit "$owner/$repo" --visibility public --accept-visibility-change-consequences
}
Write-Host ""
Write-Host "Done: https://github.com/$owner/$repo"
Write-Host "To publish a release:  git tag v2.0.0  then  git push origin v2.0.0"
