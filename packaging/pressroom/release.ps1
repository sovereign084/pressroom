# Publishes a PressRoom version on GitHub: pushes the branch, builds the installer and makes a
# release whose installer is always named PressRoom-Setup.msi, so
# https://github.com/<owner>/pressroom/releases/latest/download/PressRoom-Setup.msi is always the newest.
#   powershell -ExecutionPolicy Bypass -File packaging\pressroom\release.ps1 -Version 0.2.8 -Notes "What changed"
# Needs the GitHub CLI signed in (gh auth login) and the repository as the 'origin' remote. Each
# version must be higher than the last: the installer replaces an older PressRoom, never a newer one.
param([Parameter(Mandatory)][string]$Version, [string]$Notes = '', [switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (git -C $root status --porcelain --untracked-files=no) { throw 'Commit your changes first: the release is built from the committed code.' }
$repo = (gh repo view (git -C $root remote get-url origin) --json nameWithOwner --jq .nameWithOwner)
if (-not $repo) { throw "No GitHub repository at the 'origin' remote." }
if (gh release view "v$Version" --repo $repo --json tagName 2>$null) { throw "Version $Version is already released." }

git -C $root push origin HEAD:main
if ($LASTEXITCODE -ne 0) { throw 'git push failed' }

$msi = Join-Path $root "target-pkg\PressRoom-Setup-$Version.msi"
if (-not $SkipBuild -or -not (Test-Path $msi)) { & (Join-Path $PSScriptRoot 'build-installer.ps1') -Version $Version }
if (-not (Test-Path $msi)) { throw "The installer was not built: $msi" }
$asset = Join-Path $root 'target-pkg\PressRoom-Setup.msi'
Copy-Item $msi $asset -Force

# Notes through a file: Windows PowerShell mangles quotes in a native program's arguments.
$notesFile = Join-Path $root 'target-pkg\release-notes.md'
$body = if ($Notes) { $Notes } else { "PressRoom $Version." }
$body += "`n`n**Install or update:** download **PressRoom-Setup.msi** below and open it. If Windows says `"Windows protected your PC`", click **More info**, then **Run anyway**. A new version replaces the old one and keeps your publications and settings."
[IO.File]::WriteAllText($notesFile, $body, (New-Object Text.UTF8Encoding $false))
gh release create "v$Version" $asset --repo $repo --target main --title "PressRoom $Version" --notes-file $notesFile
if ($LASTEXITCODE -ne 0) { throw 'gh release create failed' }
"Released: https://github.com/$repo/releases/tag/v$Version"
"Download link (always the newest): https://github.com/$repo/releases/latest/download/PressRoom-Setup.msi"
