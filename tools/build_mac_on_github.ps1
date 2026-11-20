# Builds the macOS installers (Intel + Apple Silicon) on GitHub's Mac computers and downloads them to dist\.
# Started by "Build Mac versions.cmd". Uses your GitHub account and a PRIVATE repository "om-disk-analyzer".
# 'Continue': Windows PowerShell 5.1 would otherwise stop on normal messages tools print; each step checks its result
$ErrorActionPreference = 'Continue'
$app = Split-Path -Parent $PSScriptRoot
Set-Location $app
$repo = 'om-disk-analyzer'

function Say($text) { Write-Host ""; Write-Host "== $text" -ForegroundColor Cyan }

$gh = (Get-Command gh -ErrorAction SilentlyContinue).Source
if (-not $gh) { $gh = Join-Path $env:USERPROFILE 'gh\bin\gh.exe' }
if (-not (Test-Path $gh)) {
    Write-Host "GitHub CLI is needed: install it from https://cli.github.com and run this file again."
    exit 1
}
& $gh auth status *> $null
if ($LASTEXITCODE -ne 0) { Say "Sign in to GitHub"; & $gh auth login --web --scopes workflow }
$owner = (& $gh api user --jq .login).Trim()

Say "Preparing the source code"
if (-not (Test-Path .git)) {
    git init -q -b main
    git config user.name "OM Industries Inc"
    git config user.email "$owner@users.noreply.github.com"
}
git add -A
git diff --cached --quiet
if ($LASTEXITCODE -ne 0) { git commit -q -m "Update OM Disk Analyzer" }

Say "Uploading to your private repository $owner/$repo"
& $gh repo view "$owner/$repo" *> $null
if ($LASTEXITCODE -ne 0) {
    & $gh repo create $repo --private --description "OM Disk Analyzer - OM Industries Inc" --source . --remote origin --push
} else {
    git remote get-url origin *> $null
    if ($LASTEXITCODE -ne 0) { git remote add origin "https://github.com/$owner/$repo.git" }
    git push -u origin main
}
if ($LASTEXITCODE -ne 0) { throw "The upload did not work." }

Say "Starting the Mac build (about 10 minutes)"
$started = $false
for ($i = 0; $i -lt 12 -and -not $started; $i++) {
    Start-Sleep -Seconds 5
    & $gh workflow run build.yml --repo "$owner/$repo" --ref main 2>$null
    $started = ($LASTEXITCODE -eq 0)
}
if (-not $started) { throw "GitHub did not start the build. Open https://github.com/$owner/$repo/actions and press 'Run workflow'." }
Start-Sleep -Seconds 10
$run = (& $gh run list --repo "$owner/$repo" --workflow build.yml --limit 1 --json databaseId --jq '.[0].databaseId').Trim()
& $gh run watch $run --repo "$owner/$repo" --exit-status
if ($LASTEXITCODE -ne 0) { throw "The build failed. Details: https://github.com/$owner/$repo/actions/runs/$run" }

Say "Downloading the Mac installers"
$tmp = Join-Path $app 'dist\from-github'
if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
& $gh run download $run --repo "$owner/$repo" --dir $tmp
Get-ChildItem $tmp -Recurse -Filter *.dmg | ForEach-Object { Copy-Item $_.FullName (Join-Path $app 'dist') -Force; Write-Host "  $($_.Name)" }
Remove-Item -Recurse -Force $tmp
Say "Done - the .dmg files are in the dist folder"
Start-Process explorer.exe (Join-Path $app 'dist')
