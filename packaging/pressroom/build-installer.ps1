# Build PressRoom and its installer in one go.
#   powershell -ExecutionPolicy Bypass -File packaging\pressroom\build-installer.ps1 -Version 0.2.0
# Needs Rust (rustup) and WiX 5 (dotnet tool install --global wix --version 5.0.2).
# Writes target-pkg\PressRoom-Setup-<version>.msi.
param([Parameter(Mandatory)][string]$Version, [string]$TargetDir = 'C:\Users\jayde\Craft\src\target-dc')
# Native tools write progress to stderr; failures are judged by their exit codes.
$ErrorActionPreference = 'Continue'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$env:Path += ";$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\.dotnet\tools"
$env:CARGO_TARGET_DIR = $TargetDir
Push-Location $root
try {
    cargo build --release -p designcraft -p designcraft-cli 2>&1 | Select-Object -Last 2 | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    $stage = Join-Path $root 'target-pkg\bin'
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item (Join-Path $TargetDir 'release\designcraft.exe') (Join-Path $stage 'PressRoom.exe') -Force
    Copy-Item (Join-Path $TargetDir 'release\designcraft-cli.exe') (Join-Path $stage 'PressRoom-cli.exe') -Force
    Copy-Item (Join-Path $root 'tools\pub-import\pub2pressroom.ps1') $stage -Force
    Copy-Item (Join-Path $root 'NOTICE'), (Join-Path $root 'LICENSE-APACHE'), (Join-Path $root 'LICENSE-MIT') $stage -Force
    $msi = Join-Path $root "target-pkg\PressRoom-Setup-$Version.msi"
    Push-Location $PSScriptRoot
    try { wix build pressroom.wxs -d "Version=$Version" -d "BinDir=$stage" -o $msi } finally { Pop-Location }
    if ($LASTEXITCODE -ne 0) { throw 'wix build failed' }
    Get-Item $msi | Select-Object Name, Length
}
finally { Pop-Location }
