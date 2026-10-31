# Build PressRoom and its installer in one go.
#   powershell -ExecutionPolicy Bypass -File packaging\pressroom\build-installer.ps1 -Version 0.2.0
# Needs Rust (rustup), WiX 5 (dotnet tool install --global wix --version 5.0.2) and, the first
# time, the internet (the free fonts in fonts.json).
# Writes target-pkg\PressRoom-Setup-<version>.msi.
# Signing (optional, see SIGNING.md): when PRESSROOM_SIGN holds a command with {file} in it, the
# programs and then the installer are signed with it, e.g.
#   $env:PRESSROOM_SIGN = 'signtool sign /fd SHA256 /tr http://timestamp.acs.microsoft.com /td SHA256 /dlib C:\Tools\signing\Azure.CodeSigning.Dlib.dll /dmdf C:\Tools\signing\metadata.json "{file}"'
param([Parameter(Mandatory)][string]$Version, [string]$TargetDir = 'C:\Users\jayde\Craft\src\target-dc')
# Native tools write progress to stderr; failures are judged by their exit codes.
$ErrorActionPreference = 'Continue'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')

# Signs `file` with the PRESSROOM_SIGN command, when there is one.
function Sign-File([string]$file) {
    if (-not $env:PRESSROOM_SIGN) { return }
    if ($env:PRESSROOM_SIGN -notmatch '\{file\}') { throw 'PRESSROOM_SIGN needs {file} where the file goes' }
    # Through a .cmd file: PowerShell 5.1 mangles quotes handed to a native program.
    $bat = Join-Path $env:TEMP "pressroom-sign-$PID.cmd"
    Set-Content -LiteralPath $bat -Value ('@' + $env:PRESSROOM_SIGN.Replace('{file}', $file)) -Encoding ASCII
    & $bat 2>&1 | Select-Object -Last 3 | Out-Host
    $code = $LASTEXITCODE
    Remove-Item -LiteralPath $bat -ErrorAction SilentlyContinue
    if ($code -ne 0) { throw "signing $file failed" }
}

# A GUID that stays the same from build to build for `name` (a WiX component's).
function Stable-Guid([string]$name) {
    $b = [Security.Cryptography.MD5]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes("PressRoom component: $name"))
    $b[6] = ($b[6] -band 0x0f) -bor 0x30
    $b[8] = ($b[8] -band 0x3f) -bor 0x80
    return ([guid]::new([byte[]]$b)).ToString().ToUpper()
}

# The free fonts in fonts.json: downloaded once into target-pkg\fonts, each checked against its
# SHA-256 (font files are never committed), and a WiX fragment that installs them as the user's
# own Windows fonts, with their licences beside PressRoom. Returns the fragment's path.
function Get-Fonts {
    $dir = Join-Path $root 'target-pkg\fonts'
    New-Item -ItemType Directory -Force $dir | Out-Null
    $list = Get-Content (Join-Path $PSScriptRoot 'fonts.json') -Raw | ConvertFrom-Json
    foreach ($f in @($list.fonts) + @($list.licences)) {
        $path = Join-Path $dir $f.file
        if ((Test-Path $path) -and (Get-FileHash $path -Algorithm SHA256).Hash -eq $f.sha256) { continue }
        Invoke-WebRequest -Uri $f.url -UserAgent 'curl/8.4.0' -UseBasicParsing -OutFile $path
        if ((Get-FileHash $path -Algorithm SHA256).Hash -ne $f.sha256) {
            Remove-Item $path -Force
            throw "$($f.file): what $($f.url) sent doesn't match its SHA-256 in fonts.json"
        }
    }
    $fonts = foreach ($f in $list.fonts) {
        $id = ($f.file -replace '[^A-Za-z0-9]', '_')
        $name = if ($f.style -eq 'Regular') { "$($f.family) (TrueType)" } else { "$($f.family) $($f.style) (TrueType)" }
        @"
      <Component Id="Font_$id" Directory="UserFontsFolder" Guid="$(Stable-Guid $f.file)">
        <File Id="FontFile_$id" Source="`$(var.FontDir)\$($f.file)" Name="$($f.file)" />
        <RegistryValue Root="HKCU" Key="Software\Microsoft\Windows NT\CurrentVersion\Fonts" Name="$name" Value="[#FontFile_$id]" Type="string" KeyPath="yes" />
      </Component>
"@
    }
    $licences = foreach ($l in $list.licences) { "        <File Id=`"Licence_$($l.file -replace '[^A-Za-z0-9]', '_')`" Source=`"`$(var.FontDir)\$($l.file)`" Name=`"$($l.file)`" />" }
    $wxs = @"
<?xml version="1.0" encoding="utf-8"?>
<!-- Made by build-installer.ps1 from fonts.json: the free fonts PressRoom installs for the user. -->
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Fragment>
    <StandardDirectory Id="LocalAppDataFolder">
      <Directory Id="UserMicrosoftFolder" Name="Microsoft">
        <Directory Id="UserWindowsFolder" Name="Windows">
          <Directory Id="UserFontsFolder" Name="Fonts" />
        </Directory>
      </Directory>
    </StandardDirectory>
    <DirectoryRef Id="INSTALLFOLDER">
      <Directory Id="FontLicencesFolder" Name="Font licences" />
    </DirectoryRef>
    <ComponentGroup Id="PressRoomFonts">
$($fonts -join "`n")
      <Component Id="FontLicences" Directory="FontLicencesFolder" Guid="$(Stable-Guid 'Font licences')">
$($licences -join "`n")
        <RemoveFolder Id="RemoveFontLicencesFolder" On="uninstall" />
        <RegistryValue Root="HKCU" Key="Software\PressRoom" Name="FontLicences" Value="1" Type="integer" KeyPath="yes" />
      </Component>
    </ComponentGroup>
  </Fragment>
</Wix>
"@
    $out = Join-Path $root 'target-pkg\fonts.wxs'
    [IO.File]::WriteAllText($out, $wxs, (New-Object Text.UTF8Encoding $false))
    return $out
}
# Windows Installer can't read some fonts' versions (WiX gives them 0.0.0.0: Oxygen, the PT fonts),
# so it would write them again on every install and update, which fails while a program holds the
# font open; browsers do, for fonts web pages ask for (it took PressRoom off this PC on 2026-10-31).
# Those fonts are compared by their contents instead, so an unchanged one is left alone.
function Set-FontHashes([string]$msi) {
    $wi = New-Object -ComObject WindowsInstaller.Installer
    $get = { param($o, $name, $i) $o.GetType().InvokeMember($name, 'GetProperty', $null, $o, @($i)) }
    $set = { param($o, $name, $i, $v) [void]$o.GetType().InvokeMember($name, 'SetProperty', $null, $o, @($i, $v)) }
    $db = $wi.OpenDatabase($msi, 1)
    try {
        $view = $db.OpenView("SELECT ``File``, ``FileName`` FROM ``File`` WHERE ``Version`` = '0.0.0.0'")
        $view.Execute()
        $found = New-Object System.Collections.Generic.List[object]
        while ($r = $view.Fetch()) { $found.Add(@((& $get $r 'StringData' 1), ((& $get $r 'StringData' 2) -split '\|')[-1])) }
        $view.Close()
        $hashes = $db.OpenView('SELECT * FROM `MsiFileHash`')
        $hashes.Execute()
        foreach ($f in $found) {
            $one = $db.OpenView("SELECT ``Version`` FROM ``File`` WHERE ``File`` = '$($f[0])'")
            $one.Execute()
            $r = $one.Fetch()
            & $set $r 'StringData' 1 ''
            $one.Modify(2, $r)
            $one.Close()
            $h = $wi.FileHash((Join-Path $root "target-pkg\fonts\$($f[1])"), 0)
            $row = $wi.CreateRecord(6)
            & $set $row 'StringData' 1 $f[0]
            & $set $row 'IntegerData' 2 0
            for ($i = 1; $i -le 4; $i++) { & $set $row 'IntegerData' ($i + 2) (& $get $h 'IntegerData' $i) }
            $hashes.Modify(1, $row)
        }
        $hashes.Close()
        $db.Commit()
        "$($found.Count) fonts compared by contents"
    } finally { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($db); [GC]::Collect(); [GC]::WaitForPendingFinalizers() }
}
$env:Path += ";$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\.dotnet\tools"
$env:CARGO_TARGET_DIR = $TargetDir
# PressRoom's own version, built into the app: it compares it with the newest release to offer
# updates (crates/ui-egui/src/pressroom/update.rs) and shows it in File > Account.
$env:PRESSROOM_VERSION = $Version
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
    Sign-File (Join-Path $stage 'PressRoom.exe')
    Sign-File (Join-Path $stage 'PressRoom-cli.exe')
    $fontsWxs = Get-Fonts
    $msi = Join-Path $root "target-pkg\PressRoom-Setup-$Version.msi"
    Push-Location $PSScriptRoot
    try { wix build pressroom.wxs $fontsWxs -d "Version=$Version" -d "BinDir=$stage" -d "FontDir=$(Join-Path $root 'target-pkg\fonts')" -o $msi } finally { Pop-Location }
    if ($LASTEXITCODE -ne 0) { throw 'wix build failed' }
    Set-FontHashes $msi
    Sign-File $msi
    Get-Item $msi | Select-Object Name, Length
}
finally { Pop-Location }
