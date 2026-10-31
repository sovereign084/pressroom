<#
.SYNOPSIS
Convert a Microsoft Publisher file (.pub) into a PressRoom publication (.pressroom).

.DESCRIPTION
Runs on a Windows PC that has Publisher installed. Publisher itself opens the .pub (read-only) and
reports every page, text box, picture and shape; this script rebuilds them with PressRoom's
command-line tool:
  - text boxes keep their text, fonts, sizes, bold/italic/underline, ALL CAPS and small caps,
    character spacing, colours, paragraph alignment and spacing, drop caps, page numbers, inner
    margins, columns and vertical alignment; linked text boxes stay linked;
  - pictures are saved at 300 dpi and placed in picture boxes of the same size and place;
  - rectangles, rounded rectangles and ovals stay editable shapes with their fill and outline, and
    lines stay lines with their arrowheads;
  - PANTONE-style inks stay inks: spot colours of the same name and CMYK recipe as in Publisher's
    PDFs, so PressRoom's PDFs match Publisher's; a shape with a 3-D bevel takes the shade
    Publisher draws its face in;
  - objects stack in the same order, and text wraps around the objects it wrapped around in
    Publisher (only objects in front of a text box wrap its text, as in Publisher);
  - tables stay editable tables: the same columns, rows, text, fonts, fills, borders and cell
    margins (a cell keeps one formatting: its first character's when Publisher's was mixed);
  - anything else (WordArt, fancy shapes) is saved by Publisher as a picture, so it looks the
    same even though it is no longer editable;
  - objects Publisher itself can't draw (some files carry invisible leftovers) are left out;
  - the master page's objects go on PressRoom's master page.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File pub2pressroom.ps1 -In "Bulletin Oct 26.pub"
Writes "Bulletin Oct 26.pressroom" beside it, with its pictures inside.
#>
param(
    [string]$In,
    [string]$Out,
    [string]$Cli = (Join-Path $PSScriptRoot 'PressRoom-cli.exe'),
    # Also export Publisher's own PDF of the file, to compare against.
    [switch]$Reference,
    # Keep the pictures folder and the step list (left out once the publication is made).
    [switch]$KeepWork,
    # Leave the default printer alone while Publisher reads (see Use-PdfPrinter).
    [switch]$NoPrinterSwap,
    # Only put back the default printer a stopped conversion left switched, then quit.
    [switch]$RestorePrinter
)
$ErrorActionPreference = 'Stop'

# Progress for PressRoom, which shows it while the file comes in: one line a step.
function Say([string]$text) {
    [Console]::Out.WriteLine("PRESSROOM: $text")
    [Console]::Out.Flush()
}

# Publisher lays a publication out for the default printer, and waits without end for one it can't
# reach (an office printer away from the office). While it reads, the default is Microsoft Print to
# PDF, which every Windows has; the user's own default comes back afterwards. What to put back is
# kept in a file too, so a conversion that was stopped part-way is put right by the next one (or by
# -RestorePrinter, which PressRoom runs when it stops one).
$winKey = 'HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Windows'
$devKey = 'HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Devices'
$savedFile = Join-Path $env:TEMP 'PressRoom-default-printer.json'
function Restore-Printer {
    if (-not (Test-Path $savedFile)) { return }
    try {
        $s = Get-Content -Raw $savedFile | ConvertFrom-Json
        if ($s.device) { Set-ItemProperty $winKey -Name Device -Value $s.device } else { Remove-ItemProperty $winKey -Name Device -ErrorAction SilentlyContinue }
        if ($s.hadLegacy) { Set-ItemProperty $winKey -Name LegacyDefaultPrinterMode -Value ([int]$s.legacy) -Type DWord }
        else { Remove-ItemProperty $winKey -Name LegacyDefaultPrinterMode -ErrorAction SilentlyContinue }
    } finally { Remove-Item $savedFile -Force -ErrorAction SilentlyContinue }
}
function Use-PdfPrinter {
    if ($NoPrinterSwap) { return }
    $port = (Get-ItemProperty $devKey -Name 'Microsoft Print to PDF' -ErrorAction SilentlyContinue).'Microsoft Print to PDF'
    $w = Get-ItemProperty $winKey -ErrorAction SilentlyContinue
    if (-not $port -or -not $w) { return }
    if ("$($w.Device)" -like 'Microsoft Print to PDF,*') { return }
    $saved = [ordered]@{ device = $w.Device; legacy = $w.LegacyDefaultPrinterMode; hadLegacy = $null -ne $w.PSObject.Properties['LegacyDefaultPrinterMode'] }
    [IO.File]::WriteAllText($savedFile, (ConvertTo-Json $saved -Compress))
    # Windows mustn't pick another default meanwhile.
    Set-ItemProperty $winKey -Name LegacyDefaultPrinterMode -Value 1 -Type DWord
    Set-ItemProperty $winKey -Name Device -Value "Microsoft Print to PDF,$port"
}
Restore-Printer
if ($RestorePrinter) { return }
if (-not $In) { throw 'Give -In: a Publisher file or a folder of them.' }
Add-Type -AssemblyName PresentationCore
Add-Type -AssemblyName System.Drawing
$In = (Resolve-Path $In).Path
# A folder: every .pub in it (and below), each beside its original.
if (Test-Path $In -PathType Container) {
    $files = Get-ChildItem $In -Recurse -Filter *.pub | Where-Object { -not $_.PSIsContainer }
    foreach ($file in $files) {
        try { & $PSCommandPath -In $file.FullName -Cli $Cli -Reference:$Reference -KeepWork:$KeepWork } catch { "{0}: FAILED - {1}" -f $file.Name, $_.Exception.Message }
    }
    "Converted {0} Publisher file(s) in {1}" -f $files.Count, $In
    return
}
if (-not $Out) { $Out = [IO.Path]::ChangeExtension($In, '.pressroom') }
if (-not (Test-Path $Cli)) {
    # Beside the script under its build name, or in PressRoom's install folder.
    $alt = @((Join-Path $PSScriptRoot 'designcraft-cli.exe'), (Join-Path $env:LOCALAPPDATA 'Programs\PressRoom\PressRoom-cli.exe'), 'C:\Users\jayde\Craft\src\target-dc\release\designcraft-cli.exe') | Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($alt) { $Cli = $alt } else { throw "PressRoom's command-line tool was not found at $Cli" }
}
$base = [IO.Path]::Combine([IO.Path]::GetDirectoryName($Out), [IO.Path]::GetFileNameWithoutExtension($Out))
$picDir = "$base pictures"
New-Item -ItemType Directory -Force $picDir | Out-Null

$steps = New-Object System.Collections.Generic.List[object]
function Add-Step([string]$command, $params) {
    $steps.Add([ordered]@{ command = $command; params = $params })
    return $steps.Count - 1
}
function Ref([int]$step, [string]$path) { return "`$$step.$path" }

function Hex([int]$rgb) {
    $r = $rgb -band 0xFF; $g = ($rgb -shr 8) -band 0xFF; $b = ($rgb -shr 16) -band 0xFF
    return '#{0:x2}{1:x2}{2:x2}' -f $r, $g, $b
}

# Publisher's text: \r ends a paragraph, \v is a line break. PressRoom: \n and U+2028.
function Convert-Text([string]$t) {
    return $t.Replace("`r", "`n").Replace([string][char]11, [string][char]0x2028).Replace([string][char]0x0B, [string][char]0x2028)
}

$utf8 = New-Object System.Text.UTF8Encoding $false
function ByteOffset([string]$text, [int]$chars) {
    if ($chars -le 0) { return 0 }
    if ($chars -ge $text.Length) { return $utf8.GetByteCount($text) }
    return $utf8.GetByteCount($text.Substring(0, $chars))
}

function Style-Name($bold, $italic) {
    if ($bold -and $italic) { return 'Bold Italic' }
    if ($bold) { return 'Bold' }
    if ($italic) { return 'Italic' }
    return 'Regular'
}

# Fonts: Publisher names them as Windows does ("Arial Narrow", bold), PressRoom by their typographic
# family and style ("Arial", "Narrow Bold"). The installed font files say which is which.
$en = [Globalization.CultureInfo]'en-US'
$fontFaces = @{}
$fontRegistry = $null
function En-Name($names) {
    $v = $null
    try { $v = $names[$en] } catch {}
    if (-not $v) { $v = @($names.Values) | Select-Object -First 1 }
    return [string]$v
}
function Font-Faces([string]$win32) {
    $key = $win32.ToLowerInvariant()
    if ($fontFaces.ContainsKey($key)) { return ,$fontFaces[$key] }
    if ($null -eq $script:fontRegistry) {
        $script:fontRegistry = New-Object System.Collections.Generic.List[object]
        foreach ($k in 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts', 'HKCU:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts') {
            $p = Get-ItemProperty $k -ErrorAction SilentlyContinue
            if ($p) { $p.PSObject.Properties | Where-Object { $_.Name -notlike 'PS*' } | ForEach-Object { $script:fontRegistry.Add(@($_.Name, [string]$_.Value)) } }
        }
    }
    $faces = New-Object System.Collections.Generic.List[object]
    foreach ($r in $script:fontRegistry) {
        if (-not $r[0].StartsWith($win32, [StringComparison]::OrdinalIgnoreCase)) { continue }
        $file = if ([IO.Path]::IsPathRooted($r[1])) { $r[1] } else { Join-Path $env:WINDIR "Fonts\$($r[1])" }
        try {
            $g = New-Object Windows.Media.GlyphTypeface ([Uri]$file)
            if ((En-Name $g.Win32FamilyNames) -ine $win32) { continue }
            $faces.Add([pscustomobject]@{
                Family = (En-Name $g.FamilyNames); Style = (En-Name $g.FaceNames); Win32Style = (En-Name $g.Win32FaceNames)
                Weight = $g.Weight.ToOpenTypeWeight(); Italic = ($g.Style -ne [Windows.FontStyles]::Normal); File = $file
            })
        } catch {}
    }
    $fontFaces[$key] = $faces
    return ,$faces
}

# Publisher's single line spacing: the font's typographic ascender, descender and line gap (its
# OS/2 table) as a multiple of its size. Arial: 1.09, Times New Roman: 1.06, Calibri: 1.22.
$lineFactors = @{}
function Typo-Factor([string]$file) {
    if ($lineFactors.ContainsKey($file)) { return $lineFactors[$file] }
    $factor = $null
    try {
        $b = [IO.File]::ReadAllBytes($file)
        $u16 = { param($i) ([int]$b[$i] -shl 8) -bor [int]$b[$i + 1] }
        $s16 = { param($i) $v = & $u16 $i; if ($v -ge 0x8000) { $v - 0x10000 } else { $v } }
        $u32 = { param($i) ([long](& $u16 $i) -shl 16) -bor [long](& $u16 ($i + 2)) }
        $off = 0
        if ([Text.Encoding]::ASCII.GetString($b, 0, 4) -eq 'ttcf') { $off = & $u32 12 }
        $upem = 0; $asc = $null
        for ($t = 0; $t -lt (& $u16 ($off + 4)); $t++) {
            $rec = $off + 12 + 16 * $t
            $tag = [Text.Encoding]::ASCII.GetString($b, $rec, 4)
            $at = & $u32 ($rec + 8)
            if ($tag -eq 'head') { $upem = & $u16 ($at + 18) }
            if ($tag -eq 'OS/2') { $asc = & $s16 ($at + 68); $desc = & $s16 ($at + 70); $gap = & $s16 ($at + 72) }
        }
        if ($upem -gt 0 -and $null -ne $asc) { $f = ($asc - $desc + $gap) / $upem; if ($f -gt 0.5 -and $f -lt 3) { $factor = $f } }
    } catch {}
    $lineFactors[$file] = $factor
    return $factor
}

# PressRoom's (family, style) for a Publisher font name with bold and italic, and the font's line
# spacing factor.
function Map-Font([string]$name, [bool]$bold, [bool]$italic) {
    $want = Style-Name $bold $italic
    $faces = Font-Faces $name
    if ($faces.Count -eq 0) {
        # Not installed for every program: Microsoft 365's cloud fonts are only Office's to draw,
        # but their files still give the line spacing (Kermit Expanded is in the Kermit folder).
        $factor = 1.2
        $clouds = Join-Path $env:LOCALAPPDATA 'Microsoft\FontCache\4\CloudFonts'
        $folder = Get-ChildItem $clouds -Directory -ErrorAction SilentlyContinue | Where-Object { $name.StartsWith($_.Name, [StringComparison]::OrdinalIgnoreCase) } | Sort-Object { $_.Name.Length } -Descending | Select-Object -First 1
        $file = if ($folder) { Get-ChildItem $folder.FullName -Filter *.ttf -ErrorAction SilentlyContinue | Select-Object -First 1 }
        if ($file) { $f = Typo-Factor $file.FullName; if ($f) { $factor = $f } }
        return @($name, $want, $factor)
    }
    $face = $faces | Where-Object { $_.Win32Style -ieq $want } | Select-Object -First 1
    if (-not $face) {
        # No face of that name: the nearest in weight and slant (Lucida Bright's bold is Demibold).
        $w = if ($bold) { 700 } else { 400 }
        $face = $faces | Sort-Object { [Math]::Abs($_.Weight - $w) + $(if ($_.Italic -ne $italic) { 1000 } else { 0 }) } | Select-Object -First 1
    }
    $factor = Typo-Factor $face.File
    if (-not $factor) { $factor = 1.2 }
    return @($face.Family, $face.Style, $factor)
}

# Character formatting of a range, or $null when it is mixed.
function Read-Font($range) {
    $f = $range.Font
    $name = $f.Name; $size = $f.Size; $b = $f.Bold; $i = $f.Italic; $u = $f.Underline
    if (-not $name -or $size -le 0 -or $b -eq -2 -or $i -eq -2 -or ($u -ne 0 -and $u -ne 1 -and $u -ne -1)) { return $null }
    $caps = 0; $small = 0; $trk = 100.0; $kern = 0.0; $scale = 100.0
    try { $caps = [int]$f.AllCaps; $small = [int]$f.SmallCaps } catch {}
    try { $trk = [double]$f.Tracking; $kern = [double]$f.Kerning; $scale = [double]$f.Scaling } catch {}
    if ($caps -eq -2 -or $small -eq -2 -or $trk -lt -1000 -or $kern -lt -1000 -or $scale -le 0) { return $null }
    return [pscustomobject]@{
        Name = $name; Size = [double]$size; Bold = ($b -eq -1); Italic = ($i -eq -1); Underline = ($u -ne 0); Color = [int]$f.Color.RGB
        Ink = (Ink-For $f.Color); Caps = ($caps -eq -1); Small = ($small -eq -1); Tracking = $trk; Kerning = $kern; Scaling = $scale
    }
}

function Font-Key($f) {
    return '{0}|{1}|{2}|{3}|{4}|{5}|{6}|{7}|{8}|{9}|{10}' -f $f.Name, $f.Size, $f.Bold, $f.Italic, $f.Underline, $f.Color, $f.Caps, $f.Small, $f.Tracking, $f.Kerning, $f.Scaling
}

# Runs of uniform formatting in [start, start+len) of the story: (offset, length, font).
function Get-Runs($storyRange, [int]$start, [int]$len) {
    $runs = New-Object System.Collections.Generic.List[object]
    if ($len -le 0) { return ,$runs }
    $whole = $storyRange.Characters($start + 1, $len)
    $f = Read-Font $whole
    # One colour across the range: the first and last characters agree with the whole.
    if ($f) {
        $c1 = [int]$storyRange.Characters($start + 1, 1).Font.Color.RGB
        $c2 = [int]$storyRange.Characters($start + $len, 1).Font.Color.RGB
        if ($c1 -eq $f.Color -and $c2 -eq $f.Color) { $runs.Add(@($start, $len, $f)); return ,$runs }
    }
    # Word by word, and character by character inside a mixed word.
    $pos = $start
    $end = $start + $len
    while ($pos -lt $end) {
        $w = $storyRange.Characters($pos + 1, 1).Words(1, 1)
        $wStart = [Math]::Max($pos, [int]$w.Start - [int]$storyRange.Start)
        $wLen = [Math]::Min([int]$w.Length, $end - $wStart)
        if ($wLen -le 0) { $wLen = 1 }
        $wr = $storyRange.Characters($wStart + 1, $wLen)
        $wf = Read-Font $wr
        if ($wf) {
            $runs.Add(@($wStart, $wLen, $wf))
        } else {
            for ($k = 0; $k -lt $wLen; $k++) {
                $cf = Read-Font $storyRange.Characters($wStart + $k + 1, 1)
                if ($cf) { $runs.Add(@(($wStart + $k), 1, $cf)) }
            }
        }
        $pos = $wStart + $wLen
    }
    return ,$runs
}

# Publisher's PbParagraphAlignmentType → PressRoom's alignment.
$alignMap = @{ 0 = 'left'; 1 = 'center'; 2 = 'right'; 3 = 'leftJustified'; 4 = 'fullyJustified'; 6 = 'leftJustified'; 7 = 'leftJustified'; 8 = 'leftJustified'; 9 = 'fullyJustified'; 10 = 'centerJustified' }

# Where a page sits in PressRoom: spread index and the page's left edge in that spread.
$facing = $false
$pw = 0.0
function Page-Place([int]$abs) {
    if (-not $script:facing) { return @($abs, 0.0) }
    if ($abs -eq 0) { return @(0, 0.0) }
    $spread = [int][Math]::Floor(($abs + 1) / 2)
    $x = if ($abs % 2 -eq 1) { 0.0 } else { $script:pw }
    return @($spread, $x)
}

function Rect($shape, [double]$x) {
    return @(([double]$shape.Left + $x), [double]$shape.Top, ([double]$shape.Left + [double]$shape.Width + $x), ([double]$shape.Top + [double]$shape.Height))
}

$frameStep = @{}     # Publisher shape ID → step that created its PressRoom frame
$chains = New-Object System.Collections.Generic.List[object]   # linked stories: first text frame and its step
$picN = 0
$stats = @{ text = 0; pictures = 0; shapes = 0; lines = 0; tables = 0; mixedCells = 0; flattened = 0; linked = 0; skipped = 0 }

# The page's (or group's) shapes from back to front.
function Back-To-Front($shapes) {
    $list = New-Object System.Collections.Generic.List[object]
    for ($s = 1; $s -le $shapes.Count; $s++) {
        $sh = $shapes.Item($s)
        $z = $s
        try { $z = [int]$sh.ZOrderPosition } catch {}
        $list.Add(@($z, $s, $sh))
    }
    return $list | Sort-Object @{ Expression = { $_[0] } }, @{ Expression = { $_[1] } } | ForEach-Object { $_[2] }
}

# Can Publisher draw the shape? Some files carry leftover shapes it never shows (it can't save them
# as a picture either: "the image size is too big").
function Is-Drawable($shape) {
    $probe = Join-Path $picDir 'probe.png'
    try { $shape.SaveAsPicture($probe, 1); return $true } catch { return $false } finally { Remove-Item $probe -Force -ErrorAction SilentlyContinue }
}

function Emit-Rotation($shape, [int]$step) {
    $rot = [double]$shape.Rotation
    if ($rot -ne 0) { [void](Add-Step 'transform.rotate' @{ angle = -$rot; ids = @((Ref $step 'id')) }) }
}

# Text wrap: square and tight wrap around the object's box, top and bottom jumps it.
function Emit-Wrap($shape, [int]$k) {
    try { $w = $shape.TextWrap; $type = [int]$w.Type; $side = [int]$w.Side } catch { return }
    $mode = switch ($type) { 1 { 'boundingBox' } 2 { 'boundingBox' } 4 { 'jumpObject' } default { $null } }
    if (-not $mode) { return }
    if ($side -eq 4) { $mode = 'jumpObject' }
    # Automatic distance (the usual case) is 0.04" all round, whatever distances are stored.
    $wrap = @{ mode = $mode; ids = @((Ref $k 'id')); offset = 2.88 }
    try { if ($w.DistanceAuto -ne -1) { $wrap.offset = @([double]$w.DistanceTop, [double]$w.DistanceLeft, [double]$w.DistanceBottom, [double]$w.DistanceRight) } } catch {}
    $sideName = switch ($side) { 1 { 'leftSide' } 2 { 'rightSide' } 3 { 'largestArea' } default { $null } }
    if ($sideName -and $mode -eq 'boundingBox') { $wrap.side = $sideName }
    [void](Add-Step 'object.textWrap' $wrap)
}

# Rounded rectangles: Publisher's corner is a fraction of the shorter side.
function Emit-Corners($shape, [int]$k) {
    if ([int]$shape.AutoShapeType -ne 5) { return }
    $adj = 0.16667
    try { if ($shape.Adjustments.Count -ge 1) { $adj = [double]$shape.Adjustments.Item(1) } } catch {}
    $size = [Math]::Round($adj * [Math]::Min([double]$shape.Width, [double]$shape.Height), 2)
    if ($size -gt 0) { [void](Add-Step 'object.cornerOptions' @{ shape = 'rounded'; size = $size; ids = @((Ref $k 'id')) }) }
}

function Emit-Picture($shape, $spread, [double]$x) {
    $script:picN++
    $file = Join-Path $picDir ('picture-{0:000}.png' -f $script:picN)
    try { $shape.SaveAsPicture($file, 3) } catch { $shape.SaveAsPicture($file) }
    $rot = [double]$shape.Rotation
    # A rotated object is saved as it looks: place it in its upright bounding box.
    $r = Rect $shape $x
    if ($rot -ne 0) {
        $a = [Math]::Abs($rot * [Math]::PI / 180); $w = [double]$shape.Width; $h = [double]$shape.Height
        $bw = $w * [Math]::Abs([Math]::Cos($a)) + $h * [Math]::Abs([Math]::Sin($a)); $bh = $w * [Math]::Abs([Math]::Sin($a)) + $h * [Math]::Abs([Math]::Cos($a))
        $cx = [double]$shape.Left + $w / 2 + $x; $cy = [double]$shape.Top + $h / 2
        $r = @(($cx - $bw / 2), ($cy - $bh / 2), ($cx + $bw / 2), ($cy + $bh / 2))
    }
    $k = Add-Step 'frame.create' @{ spread = $spread; rect = $r; content = 'graphic' }
    $placed = Add-Step 'file.place' @{ path = $file; frame = (Ref $k 'id') }
    # Kept inside the publication, as Publisher keeps its pictures: one file to move or email.
    [void](Add-Step 'links.embed' @{ asset = (Ref $placed 'asset') })
    [void](Add-Step 'object.fit' @{ mode = 'fitContentToFrame'; ids = @((Ref $k 'id')) })
    [void](Add-Step 'object.stroke' @{ swatch = '[None]'; ids = @((Ref $k 'id')) })
    Emit-Wrap $shape $k
    return $k
}

# Inflate the zlib stream that starts at byte `at` of a PDF (as Latin-1 text).
$latin1 = [Text.Encoding]::GetEncoding(28591)
function Inflate([byte[]]$bytes, [int]$at) {
    $out = New-Object IO.MemoryStream
    $ds = New-Object IO.Compression.DeflateStream((New-Object IO.MemoryStream($bytes, ($at + 2), ($bytes.Length - $at - 2), $false)), [IO.Compression.CompressionMode]::Decompress)
    try { $ds.CopyTo($out) } catch {} finally { $ds.Dispose() }
    return $latin1.GetString($out.ToArray())
}

function Ink-Key($cmyk) { return ($cmyk | ForEach-Object { [Math]::Round([double]$_, 3).ToString($en) }) -join ',' }

# Publisher names a colour-matching ink ("PANTONE 626 C") only in its PDFs, as a Separation with
# its CMYK recipe: read from a PDF Publisher makes of the file. Recipe key → @(name, recipe).
$inkNames = @{}
function Read-Ink-Names([string]$pdf) {
    try {
        $bytes = [IO.File]::ReadAllBytes($pdf)
        $raw = $latin1.GetString($bytes)
        # Objects packed in object streams too.
        $all = New-Object System.Text.StringBuilder $raw
        foreach ($m in [regex]::Matches($raw, '/Type\s*/ObjStm[^>]*>>\s*stream\r?\n')) { [void]$all.Append("`n").Append((Inflate $bytes ($m.Index + $m.Length))) }
        foreach ($m in [regex]::Matches($all.ToString(), '/Separation\s*/([^\s/\[\]<>()]+)\s*/DeviceCMYK\s*(\d+)\s+0\s+R')) {
            $name = [regex]::Replace($m.Groups[1].Value, '#([0-9A-Fa-f]{2})', { param($h) [string][char][Convert]::ToInt32($h.Groups[1].Value, 16) })
            $obj = [regex]::Match($raw, "(?<!\d)$($m.Groups[2].Value)\s+0\s+obj\s*<<(.*?)>>\s*stream\r?\n", 'Singleline')
            if (-not $obj.Success) { continue }
            # Publisher's tint function: {dup 0.792 mul exch dup 0.173 mul exch ...}; or C1 [c m y k].
            $v = @()
            if ($obj.Groups[1].Value -match '/C1\s*\[([^\]]*)\]') { $v = @($Matches[1].Trim() -split '\s+' | ForEach-Object { [double]::Parse($_, $en) }) }
            else {
                $fn = if ($obj.Groups[1].Value -match '/FlateDecode') { Inflate $bytes ($obj.Index + $obj.Length) } else { $raw.Substring($obj.Index + $obj.Length, [Math]::Min(400, $raw.Length - $obj.Index - $obj.Length)) }
                $v = @([regex]::Matches($fn, '([\d.]+)\s+mul') | ForEach-Object { [double]::Parse($_.Groups[1].Value, $en) })
            }
            if ($v.Count -eq 4) { $script:inkNames[(Ink-Key $v)] = @($name, $v) }
        }
    } catch { Write-Warning "Publisher's ink names could not be read: $($_.Exception.Message)" }
}

# A colour-matching (PANTONE-style) ink → its spot swatch, made once; $null for other colours.
# Publisher shows the ink on screen through its own RGB, but its PDFs carry the ink with its CMYK
# recipe, which PDF viewers and print shops show deeper; PressRoom keeps the ink. One its PDF
# doesn't name (a process ink) is a CMYK colour of that recipe, as in the PDF.
function Ink-For($cf) {
    try {
        if ([int]$cf.Type -ne 4 -or [double]$cf.TintAndShade -ne 0) { return $null }
        $c = $cf.CMYK
        $recipe = @(([double]$c.Cyan / 255), ([double]$c.Magenta / 255), ([double]$c.Yellow / 255), ([double]$c.Black / 255))
    } catch { return $null }
    $key = Ink-Key $recipe
    if ($swatches.ContainsKey("ink:$key")) { return $swatches["ink:$key"] }
    $found = $inkNames[$key]
    $p = $recipe | ForEach-Object { [Math]::Round($_ * 100, 1) }
    $name = if ($found) { $found[0] } else { 'C={0} M={1} Y={2} K={3}' -f ($p | ForEach-Object { [Math]::Round($_) }) }
    if ($found) { $p = $found[1] | ForEach-Object { [Math]::Round($_ * 100, 1) } }
    [void](Add-Step 'swatch.create' @{ name = $name; spot = [bool]$found; color = @{ c = $p[0]; m = $p[1]; y = $p[2]; k = $p[3] } })
    $swatches["ink:$key"] = $name
    return $name
}

# A 3-D bevel: Publisher lights the face, so it shows darker than its colour (the page-1 sidebar's
# grey 127 shows as 96). PressRoom draws no bevels, so the face takes the colour Publisher shows:
# the commonest opaque colour of Publisher's picture of the shape.
function Bevel-Face($shape) {
    try { $t = $shape.ThreeD; if ($t.Visible -ne -1 -or [int]$t.BevelTopType -le 1) { return $null } } catch { return $null }
    $file = Join-Path $picDir 'bevel.png'
    try {
        $shape.SaveAsPicture($file, 1)
        $bmp = New-Object System.Drawing.Bitmap $file
        try {
            $counts = @{}
            for ($i = 1; $i -lt 40; $i++) {
                for ($j = 1; $j -lt 40; $j++) {
                    $px = $bmp.GetPixel([int]($bmp.Width * $i / 40), [int]($bmp.Height * $j / 40))
                    if ($px.A -lt 255) { continue }
                    $hex = '#{0:x2}{1:x2}{2:x2}' -f $px.R, $px.G, $px.B
                    $counts[$hex] = 1 + [int]$counts[$hex]
                }
            }
        } finally { $bmp.Dispose() }
        if ($counts.Count -eq 0) { return $null }
        return ($counts.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First 1).Key
    } catch { return $null } finally { Remove-Item $file -Force -ErrorAction SilentlyContinue }
}

function Emit-Box-Style($shape, [int]$k) {
    try {
        if ($shape.Fill.Visible -eq -1) {
            $face = Bevel-Face $shape
            $ink = if ($face) { $null } else { Ink-For $shape.Fill.ForeColor }
            if ($ink) { [void](Add-Step 'object.fill' @{ swatch = $ink; ids = @((Ref $k 'id')) }) }
            else {
                $color = if ($face) { $face } else { Hex $shape.Fill.ForeColor.RGB }
                [void](Add-Step 'object.color' @{ color = $color; target = 'fill'; ids = @((Ref $k 'id')) })
            }
        }
        # A 0 pt outline draws nothing in Publisher.
        if ($shape.Line.Visible -eq -1 -and [double]$shape.Line.Weight -gt 0) {
            [void](Add-Step 'object.stroke' @{ weight = [double]$shape.Line.Weight; ids = @((Ref $k 'id')) })
            $ink = Ink-For $shape.Line.ForeColor
            if ($ink) { [void](Add-Step 'object.stroke' @{ swatch = $ink; ids = @((Ref $k 'id')) }) }
            else { [void](Add-Step 'object.color' @{ color = (Hex $shape.Line.ForeColor.RGB); target = 'stroke'; ids = @((Ref $k 'id')) }) }
        } else {
            [void](Add-Step 'object.stroke' @{ swatch = '[None]'; ids = @((Ref $k 'id')) })
        }
    } catch {}
}

function Emit-Frame-Options($tf, [int]$k) {
    $valign = switch ([int]$tf.VerticalTextAlignment) { 1 { 'center' } 2 { 'bottom' } default { 'top' } }
    $opts = @{
        inset = @([double]$tf.MarginTop, [double]$tf.MarginLeft, [double]$tf.MarginBottom, [double]$tf.MarginRight)
        verticalJustification = $valign
        ids = @((Ref $k 'id'))
    }
    try { $cols = [int]$tf.Columns; if ($cols -gt 1) { $opts.columns = $cols; $opts.gutter = [double]$tf.ColumnSpacing } } catch {}
    [void](Add-Step 'object.textFrameOptions' $opts)
}

# Character formatting of [a, b) bytes of the story, with the paragraph's line spacing (`lines` of
# the font's single spacing, or `exact` points).
function Emit-Chars($k, [int]$a, [int]$b, $f, [double]$lines, [double]$exact) {
    [void](Add-Step 'text.select' @{ story = (Ref $k 'story'); anchor = $a; focus = $b })
    Emit-Format $f $lines $exact
}

# Character formatting `f` on what is selected (text, or table cells).
function Emit-Format($f, [double]$lines, [double]$exact) {
    $fam = Map-Font $f.Name $f.Bold $f.Italic
    $lead = if ($exact -gt 0) { $exact } else { $f.Size * $fam[2] * $lines }
    $attrs = @{ fontFamily = $fam[0]; size = $f.Size; underline = $f.Underline; leading = @{ kind = 'points'; value = [Math]::Round($lead, 2) } }
    # Character spacing: Publisher's tracking (percent) and kerning (points between letters).
    $trk = [Math]::Round(($f.Tracking - 100) * 10 + $f.Kerning / $f.Size * 1000)
    if ($trk -ne 0) { $attrs.tracking = $trk }
    if ([Math]::Abs($f.Scaling - 100) -gt 0.01) { $attrs.hScale = $f.Scaling / 100 }
    if ($f.Caps) { $attrs.capitalization = 'allCaps' } elseif ($f.Small) { $attrs.capitalization = 'smallCaps' }
    [void](Add-Step 'type.char' @{ attrs = $attrs })
    [void](Add-Step 'type.char' @{ attrs = @{ fontStyle = $fam[1] } })
    if ($f.Ink) { [void](Add-Step 'type.char' @{ attrs = @{ fill = $f.Ink } }) }
    elseif ($f.Color -ne 0) { [void](Add-Step 'object.color' @{ color = (Hex $f.Color); text = $true }) }
}

# A paragraph's alignment, spacing and indents. Publisher breaks lines one at a time, never
# squeezes spaces, and hyphenates only words that cross the last quarter inch (its hyphenation
# zone).
function Para-Attrs($pf) {
    $attrs = @{ align = $alignMap[[int]$pf.Alignment]; composer = 'singleLine'; wordSpaceMin = 1.0; hyphZone = 18.0 }
    if (-not $attrs.align) { $attrs.align = 'left' }
    try { $attrs.spaceBefore = [double]$pf.SpaceBefore; $attrs.spaceAfter = [double]$pf.SpaceAfter } catch {}
    try { $attrs.leftIndent = [double]$pf.LeftIndent; $attrs.firstLineIndent = [double]$pf.FirstLineIndent } catch {}
    return $attrs
}

# Line spacing: Publisher's 'lines' (1.19 by default) or exact points → @(lines, exact).
function Line-Spacing($pf) {
    $lines = 1.0; $exact = 0.0
    try {
        switch ([int]$pf.LineSpacingRule) { 1 { $lines = 1.5 } 2 { $lines = 2.0 } 4 { $exact = [double]$pf.LineSpacing } 5 { $lines = [double]$pf.LineSpacing } }
    } catch {}
    return @($lines, $exact)
}

function Emit-Text($shape, $spread, [double]$x) {
    $tf = $shape.TextFrame
    $kind = if ([int]$shape.Type -eq 1 -and [int]$shape.AutoShapeType -eq 9) { 'ellipse' } else { 'rectangle' }
    if ($tf.HasPreviousLink -eq -1) {
        # Its text arrives through the link, threaded once every frame is made.
        $k = Add-Step 'frame.create' @{ spread = $spread; rect = (Rect $shape $x); shape = $kind; content = 'unassigned' }
        $frameStep[[int]$shape.ID] = $k
        Emit-Box-Style $shape $k
        Emit-Corners $shape $k
        Emit-Rotation $shape $k
        Emit-Wrap $shape $k
        $script:stats.linked++
        return
    }
    $range = if ($tf.HasNextLink -eq -1) { $tf.Story.TextRange } else { $tf.TextRange }
    $raw = [string]$range.Text
    # Page-number fields: Publisher keeps "#" in the text, PressRoom its page-number marker.
    try {
        $fields = $range.Fields
        for ($fi = 1; $fi -le $fields.Count; $fi++) {
            $fl = $fields.Item($fi)
            $marker = switch ([int]$fl.Type) { 1 { [string][char]0xE000 } 2 { [string][char]0xE007 } 3 { [string][char]0xE008 } default { $null } }
            if ($null -eq $marker) { continue }
            $fs = [int]$fl.TextRange.Start - [int]$range.Start
            if ([int]$fl.TextRange.Length -eq 1 -and $fs -ge 0 -and $fs -lt $raw.Length) { $raw = $raw.Substring(0, $fs) + $marker + $raw.Substring($fs + 1) }
        }
    } catch {}
    $trim = $raw.TrimEnd("`r")
    $text = Convert-Text $trim
    $k = Add-Step 'frame.create' @{ spread = $spread; rect = (Rect $shape $x); shape = $kind; content = 'text'; text = $text }
    $frameStep[[int]$shape.ID] = $k
    if ($tf.HasNextLink -eq -1) { $chains.Add(@($tf, $k)) }
    $script:stats.text++
    Emit-Frame-Options $tf $k
    Emit-Box-Style $shape $k
    Emit-Corners $shape $k
    Emit-Rotation $shape $k
    Emit-Wrap $shape $k
    # Paragraphs.
    $paraCount = ($trim.Split("`r")).Count
    $rangeStart = [int]$range.Start
    for ($p = 1; $p -le $paraCount; $p++) {
        $pr = $range.Paragraphs($p, 1)
        $ps = [int]$pr.Start - $rangeStart
        $pl = [Math]::Min([int]$pr.Length, $trim.Length - $ps)
        if ($ps -ge $trim.Length) { break }
        if ($pl -lt 0) { $pl = 0 }
        $a = ByteOffset $text $ps; $b = ByteOffset $text ($ps + $pl)
        $pf = $pr.ParagraphFormat
        $attrs = Para-Attrs $pf
        # Line and paragraph breaks: Publisher's keep options and widow control.
        $keepFirst = 0; $keepLast = 0
        try {
            if ($pf.KeepWithNext -eq -1) { $attrs.keepWithNext = 1 }
            if ($pf.KeepLinesTogether -eq -1) { $attrs.keepLinesTogether = $true; $attrs.keepAllLines = $true }
            if ($pf.WidowControl -eq -1) { $keepFirst = 2; $keepLast = 2 }
        } catch {}
        # Drop cap: how many lines it drops and how many letters it takes. Publisher never starts
        # one where its lines don't fit.
        $dc = $null
        try { $d = $pr.DropCap; if ([int]$d.Size -gt 1 -and [int]$d.Span -gt 0) { $dc = $d; $attrs.dropCapLines = [int]$d.Size; $attrs.dropCapChars = [int]$d.Span; $keepFirst = [Math]::Max($keepFirst, [int]$d.Size); $keepLast = [Math]::Max($keepLast, 1) } } catch {}
        if ($keepFirst -gt 0 -and -not $attrs.keepAllLines) { $attrs.keepLinesTogether = $true; $attrs.keepAllLines = $false; $attrs.keepFirst = $keepFirst; $attrs.keepLast = $keepLast }
        $spacing = Line-Spacing $pf
        $lines = $spacing[0]; $exact = $spacing[1]
        [void](Add-Step 'text.select' @{ story = (Ref $k 'story'); anchor = $a; focus = $b })
        [void](Add-Step 'type.para' @{ attrs = $attrs })
        # Character runs, merged where the formatting repeats.
        $runs = Get-Runs $range $ps ([Math]::Max(1, $pl))
        $merged = New-Object System.Collections.Generic.List[object]
        foreach ($r in $runs) {
            if ($merged.Count -gt 0) {
                $last = $merged[$merged.Count - 1]
                if ((Font-Key $last[2]) -eq (Font-Key $r[2]) -and ($last[0] + $last[1] -eq $r[0])) { $merged[$merged.Count - 1] = @($last[0], ($last[1] + $r[1]), $last[2]); continue }
            }
            $merged.Add($r)
        }
        foreach ($r in $merged) {
            $ra = ByteOffset $text $r[0]; $rb = ByteOffset $text ([Math]::Min($text.Length, $r[0] + $r[1]))
            if ($rb -le $ra) { continue }
            Emit-Chars $k $ra $rb $r[2] $lines $exact
        }
        # The drop cap's own font and colour.
        if ($dc -and $dc.FontName) {
            $cb = ByteOffset $text ([Math]::Min($text.Length, $ps + [int]$dc.Span))
            if ($cb -gt $a) {
                $fam = Map-Font ([string]$dc.FontName) ($dc.FontBold -eq -1) ($dc.FontItalic -eq -1)
                [void](Add-Step 'text.select' @{ story = (Ref $k 'story'); anchor = $a; focus = $cb })
                [void](Add-Step 'type.char' @{ attrs = @{ fontFamily = $fam[0] } })
                [void](Add-Step 'type.char' @{ attrs = @{ fontStyle = $fam[1] } })
                try { [void](Add-Step 'object.color' @{ color = (Hex $dc.FontColor.RGB); text = $true }) } catch {}
            }
        }
    }
    # Direction marks, inserted from the end so the offsets before them still hold.
    $marks = Bidi-Marks $range $trim
    for ($m = $marks.Count - 1; $m -ge 0; $m--) {
        $at = ByteOffset $text $marks[$m][0]
        [void](Add-Step 'text.select' @{ story = (Ref $k 'story'); anchor = $at; focus = $at })
        [void](Add-Step 'text.insert' @{ text = [string]$marks[$m][1]; raw = $true })
    }
    [void](Add-Step 'edit.deselectAll' @{})
}

# Publisher lays out mixed right-to-left and left-to-right text by its own rules, not Unicode's:
# punctuation reads in the direction of the language it was typed in, and a space in the direction
# of the character before it. "PEH — פ — THE" (dash and spaces typed in Hebrew) shows as
# "PEH —  — פTHE", and "פ + ז = פז" (plus and equals typed in Spanish) as " פ+  ז= זפ". PressRoom
# follows Unicode, so invisible direction marks (U+200F right to left, U+200E left to right) go
# where the two differ: in left-to-right paragraphs that hold right-to-left letters. Returns
# (character offset, mark) pairs in text order.
$rtlLanguages = @(0x01, 0x0D, 0x20, 0x29, 0x3D, 0x59, 0x5A, 0x63, 0x65, 0x80, 0x8C, 0x92)
$rtlLetters = [regex]('[{0}-{1}{2}-{3}{4}-{5}]' -f [char]0x0590, [char]0x08FF, [char]0xFB1D, [char]0xFDFF, [char]0xFE70, [char]0xFEFE)
function Bidi-Marks($range, [string]$text) {
    $marks = New-Object System.Collections.Generic.List[object]
    if (-not $rtlLetters.IsMatch($text)) { return ,$marks }
    $start = 0
    while ($start -lt $text.Length) {
        $end = $text.IndexOf("`r", $start)
        if ($end -lt 0) { $end = $text.Length }
        $ltr = $true
        try { $ltr = [int]$range.Characters($start + 1, 1).ParagraphFormat.TextDirection -ne 2 } catch {}
        if ($ltr -and $rtlLetters.IsMatch($text.Substring($start, $end - $start))) {
            # Letters (R, L), digits (D), and runs of spaces and punctuation (N) with Publisher's direction.
            $tokens = New-Object System.Collections.Generic.List[object]
            $prev = 'L'
            for ($i = $start; $i -lt $end; $i++) {
                $c = $text[$i]
                $cat = [int][Globalization.CharUnicodeInfo]::GetUnicodeCategory($c)
                $kind = $null; $dir = $null
                if ([int]$c -eq 0x200F -or (-not [char]::IsDigit($c) -and $rtlLetters.IsMatch([string]$c))) { $kind = 'R' }
                elseif ($cat -eq 8) { $kind = 'D' }
                elseif ($cat -eq 11 -or [int]$c -eq 9 -or [int]$c -eq 11) { $dir = $prev }
                elseif ($cat -ge 18 -and $cat -le 28) {
                    $id = 0
                    try { $id = [int]$range.Characters($i + 1, 1).LanguageID } catch {}
                    $dir = if ($rtlLanguages -contains ($id -band 0x3FF)) { 'R' } else { 'L' }
                }
                elseif ($cat -le 4 -or $cat -eq 9 -or $cat -eq 17 -or [int]$c -eq 0x200E) { $kind = 'L' }
                else { continue }
                if ($dir) {
                    $last = if ($tokens.Count -gt 0) { $tokens[$tokens.Count - 1] } else { $null }
                    if ($last -and $last.Kind -eq 'N' -and $last.Dir -eq $dir -and $last.End -eq $i) { $last.End = $i + 1 }
                    else { $tokens.Add([pscustomobject]@{ Kind = 'N'; Dir = $dir; Start = $i; End = $i + 1 }) }
                    $prev = $dir
                } else {
                    $tokens.Add([pscustomobject]@{ Kind = $kind; Start = $i; End = $i + 1 })
                    $prev = if ($kind -eq 'R') { 'R' } else { 'L' }
                }
            }
            # Unicode gives a run of spaces and punctuation the direction of the letters on both
            # sides, or left to right when they differ.
            $last = 'L'
            for ($t = 0; $t -lt $tokens.Count; $t++) {
                $tok = $tokens[$t]
                if ($tok.Kind -ne 'N') { if ($tok.Kind -ne 'D') { $last = $tok.Kind }; continue }
                $next = if ($t + 1 -lt $tokens.Count) { $tokens[$t + 1].Kind } else { 'End' }
                if ($tok.Dir -eq 'R') {
                    if ($last -ne 'R') { $marks.Add(@($tok.Start, [char]0x200F)) }
                    if ($next -ne 'R') { $marks.Add(@($tok.End, [char]0x200F)) }
                    $last = 'R'
                } elseif ($last -eq 'R' -and $next -ne 'L' -and $next -ne 'End') {
                    $marks.Add(@($tok.End, [char]0x200E))
                    $last = 'L'
                }
            }
        }
        $start = $end + 1
    }
    return ,$marks
}

# Publisher's arrowheads (MsoArrowheadStyle) → PressRoom's line ends.
function Arrow-Name([int]$style) {
    switch ($style) { 2 { 'triangle' } 3 { 'barbed' } 4 { 'barbed' } 5 { 'square' } 6 { 'circleSolid' } default { $null } }
}

function Emit-Line($shape, $spread, [double]$x) {
    $x0 = [double]$shape.Left + $x; $y0 = [double]$shape.Top
    $x1 = $x0 + [double]$shape.Width; $y1 = $y0 + [double]$shape.Height
    if ($shape.HorizontalFlip -eq -1) { $t = $x0; $x0 = $x1; $x1 = $t }
    if ($shape.VerticalFlip -eq -1) { $t = $y0; $y0 = $y1; $y1 = $t }
    $k = Add-Step 'line.create' @{ spread = $spread; a = @($x0, $y0); b = @($x1, $y1) }
    $ln = $shape.Line
    $stroke = @{ weight = [double]$ln.Weight; ids = @((Ref $k 'id')) }
    $s = Arrow-Name ([int]$ln.BeginArrowheadStyle); if ($s) { $stroke.start = $s }
    $e = Arrow-Name ([int]$ln.EndArrowheadStyle); if ($e) { $stroke.end = $e }
    $ink = Ink-For $ln.ForeColor
    if ($ink) { $stroke.swatch = $ink }
    [void](Add-Step 'object.stroke' $stroke)
    if (-not $ink) { [void](Add-Step 'object.color' @{ color = (Hex $ln.ForeColor.RGB); target = 'stroke'; ids = @((Ref $k 'id')) }) }
    $script:stats.lines++
}

# A swatch for an RGB colour (table fills and borders take swatches): black and white are the
# document's own, others are made once, named as InDesign names an RGB colour.
$swatches = @{}
function Swatch-For([int]$rgb) {
    $hex = Hex $rgb
    if ($hex -eq '#000000') { return '[Black]' }
    if ($hex -eq '#ffffff') { return '[Paper]' }
    if (-not $swatches.ContainsKey($hex)) {
        $name = 'R={0} G={1} B={2}' -f ($rgb -band 0xFF), (($rgb -shr 8) -band 0xFF), (($rgb -shr 16) -band 0xFF)
        [void](Add-Step 'swatch.create' @{ name = $name; color = $hex })
        $swatches[$hex] = $name
    }
    return $swatches[$hex]
}

# A table: a PressRoom table of the same columns, rows, text, fills, borders and cell margins, in
# a text box of its own, so it can be edited (the monthly calendar).
function Emit-Table($shape, $spread, [double]$x) {
    $t = $shape.Table
    $nr = [int]$t.Rows.Count; $nc = [int]$t.Columns.Count
    $k = Add-Step 'frame.create' @{ spread = $spread; rect = (Rect $shape $x); content = 'text'; text = '' }
    # The box grows down with the table, as Publisher's table does when a row is added.
    [void](Add-Step 'object.textFrameOptions' @{ inset = 0; autoSize = 'heightOnly'; autoSizeReference = 'topLeft'; ids = @((Ref $k 'id')) })
    [void](Add-Step 'object.stroke' @{ swatch = '[None]'; ids = @((Ref $k 'id')) })
    [void](Add-Step 'text.select' @{ story = (Ref $k 'story'); anchor = 0; focus = 0 })
    $ti = Add-Step 'table.insert' @{ rows = $nr; cols = $nc; width = [double]$shape.Width }
    $cells = { param([int]$r0, [int]$r1, [int]$c0, [int]$c1) @{ story = (Ref $k 'story'); table = (Ref $ti 'table'); rows = @($r0, $r1); cols = @($c0, $c1) } }
    # Sizes: rows grow with their text when Publisher's do.
    for ($c = 1; $c -le $nc; $c++) {
        $p = & $cells 0 ($nr - 1) ($c - 1) ($c - 1); $p.width = [double]$t.Columns.Item($c).Width
        [void](Add-Step 'table.setColumnWidth' $p)
    }
    $grow = $true
    try { $grow = $t.GrowToFitText -ne 0 } catch {}
    for ($r = 1; $r -le $nr; $r++) {
        $p = & $cells ($r - 1) ($r - 1) 0 ($nc - 1); $p.height = [double]$t.Rows.Item($r).Height; $p.mode = if ($grow) { 'atLeast' } else { 'exactly' }
        [void](Add-Step 'table.setRowHeight' $p)
    }
    # Each cell: its text, fill, margins and vertical alignment, then its formatting.
    $edges = @{}
    $put = { param([string]$key, $border) $w = 0.0; $rgb = 0; try { $w = [double]$border.Weight; $rgb = [int]$border.Color.RGB } catch {}; if (-not $edges.ContainsKey($key) -or $w -gt $edges[$key][0]) { $edges[$key] = @($w, $rgb) } }
    for ($r = 1; $r -le $nr; $r++) {
        for ($c = 1; $c -le $nc; $c++) {
            $cell = $t.Rows.Item($r).Cells.Item($c)
            $tr = $cell.TextRange
            $p = & $cells ($r - 1) ($r - 1) ($c - 1) ($c - 1)
            $text = Convert-Text (([string]$tr.Text).TrimEnd("`r"))
            if ($text) { $p.text = $text }
            try {
                $p.fill = '[None]'
                if ($cell.Fill.Visible -eq -1) { $p.fill = Ink-For $cell.Fill.ForeColor; if (-not $p.fill) { $p.fill = Swatch-For ([int]$cell.Fill.ForeColor.RGB) } }
            } catch {}
            try { $p.insets = @([double]$cell.MarginTop, [double]$cell.MarginLeft, [double]$cell.MarginBottom, [double]$cell.MarginRight) } catch {}
            try { $p.vj = switch ([int]$cell.VerticalTextAlignment) { 1 { 'center' } 2 { 'bottom' } default { 'top' } } } catch {}
            [void](Add-Step 'table.setCell' $p)
            # A cell's text takes one formatting (its first character's when it is mixed), as typed
            # into the cell from then on.
            $sel = & $cells ($r - 1) ($r - 1) ($c - 1) ($c - 1); $sel.what = 'cell'
            [void](Add-Step 'table.select' $sel)
            $f = Read-Font $tr
            if (-not $f -and [int]$tr.Length -gt 0) { $f = Read-Font $tr.Characters(1, 1); $script:stats.mixedCells++ }
            $pf = $tr.Paragraphs(1, 1).ParagraphFormat
            [void](Add-Step 'type.para' @{ attrs = (Para-Attrs $pf) })
            if ($f) { $spacing = Line-Spacing $pf; Emit-Format $f $spacing[0] $spacing[1] }
            # Borders: where two cells meet, the heavier of their two sides.
            & $put "h,$($r - 1),$($c - 1)" $cell.BorderTop
            & $put "h,$r,$($c - 1)" $cell.BorderBottom
            & $put "v,$($r - 1),$($c - 1)" $cell.BorderLeft
            & $put "v,$($r - 1),$c" $cell.BorderRight
        }
    }
    # The borders, a run of cells at a time.
    $stroke = { param($e) if ($e[0] -gt 0) { @{ weight = $e[0]; color = (Swatch-For $e[1]); edges = '' } } else { @{ weight = 0; edges = '' } } }
    $same = { param($a, $b) $a[0] -eq $b[0] -and ($a[0] -eq 0 -or $a[1] -eq $b[1]) }
    for ($line = 0; $line -le $nr; $line++) {
        $c0 = 0
        while ($c0 -lt $nc) {
            $e = $edges["h,$line,$c0"]; $c1 = $c0
            while ($c1 + 1 -lt $nc -and (& $same $edges["h,$line,$($c1 + 1)"] $e)) { $c1++ }
            $row = [Math]::Min($line, $nr - 1)
            $p = & $cells $row $row $c0 $c1; $p.stroke = & $stroke $e; $p.stroke.edges = if ($line -lt $nr) { 'top' } else { 'bottom' }
            [void](Add-Step 'table.setCell' $p)
            $c0 = $c1 + 1
        }
    }
    for ($line = 0; $line -le $nc; $line++) {
        $r0 = 0
        while ($r0 -lt $nr) {
            $e = $edges["v,$r0,$line"]; $r1 = $r0
            while ($r1 + 1 -lt $nr -and (& $same $edges["v,$($r1 + 1),$line"] $e)) { $r1++ }
            $col = [Math]::Min($line, $nc - 1)
            $p = & $cells $r0 $r1 $col $col; $p.stroke = & $stroke $e; $p.stroke.edges = if ($line -lt $nc) { 'left' } else { 'right' }
            [void](Add-Step 'table.setCell' $p)
            $r0 = $r1 + 1
        }
    }
    # The empty line the table sits on takes no room, so the box is the table's height. (Last: the
    # table commands above need the insertion point in the table.)
    [void](Add-Step 'text.select' @{ story = (Ref $k 'story'); anchor = 0; focus = 64 })
    [void](Add-Step 'type.char' @{ attrs = @{ size = 1; leading = @{ kind = 'points'; value = 0.1 } } })
    [void](Add-Step 'edit.deselectAll' @{})
    Emit-Rotation $shape $k
    Emit-Wrap $shape $k
    $script:stats.tables++
}

function Emit-Shape($shape, $spread, [double]$x) {
    $type = [int]$shape.Type
    if ($type -eq 6) {   # Group: its members, each in its own place.
        foreach ($member in (Back-To-Front $shape.GroupItems)) { Emit-Shape $member $spread $x }
        return
    }
    $ast = 0
    try { $ast = [int]$shape.AutoShapeType } catch {}
    $isLine = $type -eq 9
    if ($type -eq 1) { try { $isLine = ($shape.Connector -eq -1) -or ($ast -eq -2 -and ([double]$shape.Width -eq 0 -or [double]$shape.Height -eq 0)) } catch {} }
    if ($isLine) { Emit-Line $shape $spread $x; return }
    $isTable = $type -eq 18
    try { $isTable = $isTable -or $shape.HasTable -eq -1 } catch {}
    if ($isTable) {
        # A table Publisher can't describe is taken back and kept as its picture.
        $mark = $steps.Count
        $known = @($swatches.Keys)
        try { Emit-Table $shape $spread $x; return } catch {
            while ($steps.Count -gt $mark) { $steps.RemoveAt($steps.Count - 1) }
            foreach ($hex in @($swatches.Keys)) { if ($known -notcontains $hex) { $swatches.Remove($hex) } }
            Write-Warning "A table on this page couldn't be read, so it is kept as a picture: $($_.Exception.Message)"
        }
    }
    $hasText = $false
    try { $hasText = ($shape.HasTextFrame -eq -1) -and ($type -eq 17 -or [int]$shape.TextFrame.Story.TextRange.Length -gt 1) } catch {}
    if ($hasText) { Emit-Text $shape $spread $x; return }
    if ($type -eq 1 -and -not (Is-Drawable $shape)) { $script:stats.skipped++; return }
    if ($type -eq 1 -and @(1, 5, 9) -contains $ast) {
        $kind = if ($ast -eq 9) { 'ellipse' } else { 'rectangle' }
        $k = Add-Step 'frame.create' @{ spread = $spread; rect = (Rect $shape $x); shape = $kind; content = 'unassigned' }
        Emit-Box-Style $shape $k
        Emit-Corners $shape $k
        Emit-Rotation $shape $k
        Emit-Wrap $shape $k
        $script:stats.shapes++
        return
    }
    # Pictures (embedded 13, linked 11, placeholders 14); anything else is kept as its picture.
    if (@(11, 13, 14) -contains $type) { $script:stats.pictures++ } else { $script:stats.flattened++ }
    [void](Emit-Picture $shape $spread $x)
}

# Publisher's own PDF, for its ink names (and to compare against). A Publisher of its own makes it,
# in a process of its own and against the clock: made by the Publisher that reads the file, in this
# script's process, the export can hang for good (it did every time on 2026-10-30, though the same
# export took seconds in a fresh process), and the ink names are worth having, not waiting for.
function Export-Reference([string]$pdf) {
    $job = Start-Job -ArgumentList $In, $pdf -ScriptBlock {
        param($in, $pdf)
        $a = New-Object -ComObject Publisher.Application
        try {
            $d = $a.Open($in, $true, $false, 'pbDoNotSaveChanges')
            $d.ExportAsFixedFormat(2, $pdf)
            $d.Close()
        } finally { $a.Quit() }
    }
    try {
        if (Wait-Job $job -Timeout 150) {
            Receive-Job $job -ErrorAction SilentlyContinue | Out-Null
            return (Test-Path $pdf)
        }
        Write-Warning "Publisher's own PDF took too long, so the file comes in without its ink names."
        Stop-Job $job
        # The Publisher that job started (the reading one isn't running yet).
        Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'MSPUB.EXE' -and $_.CommandLine -match 'Embedding|Automation' } |
            ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
        return $false
    } finally { Remove-Job $job -Force -ErrorAction SilentlyContinue }
}

Use-PdfPrinter
$app = $null
try {
    Say 'Publisher is reading the colours'
    $pdf = if ($Reference) { "$base (Publisher).pdf" } else { Join-Path $picDir 'inks.pdf' }
    try { if (Export-Reference $pdf) { Read-Ink-Names $pdf } } catch { Write-Warning "Publisher's own PDF could not be made: $($_.Exception.Message)" }
    if (-not $Reference) { Remove-Item $pdf -Force -ErrorAction SilentlyContinue }
    Say 'Starting Publisher'
    $app = New-Object -ComObject Publisher.Application
    Say 'Publisher is opening the file'
    $doc = $app.Open($In, $true, $false, 'pbDoNotSaveChanges')
    $pw = [double]$doc.PageSetup.PageWidth
    $ph = [double]$doc.PageSetup.PageHeight
    $n = [int]$doc.Pages.Count
    $halfLetter = [Math]::Abs($pw - 396) -lt 2 -and [Math]::Abs($ph - 612) -lt 2
    $script:facing = ($doc.ViewTwoPageSpread -eq -1) -or ($halfLetter -and $n -ge 4 -and $n % 4 -eq 0)
    $script:pw = $pw
    $margins = 36.0
    try { $margins = [double]$doc.LayoutGuides.MarginTop } catch {}
    [void](Add-Step 'file.new' @{ units = 'inches'; width = $pw; height = $ph; pages = $n; facingPages = $script:facing; margins = $margins; intent = 'print' })
    # As in Publisher: only objects in front of a text box wrap its text, and characters a font
    # lacks come from another font.
    [void](Add-Step 'document.preferences' @{ textWrapOnlyBeneath = $true; glyphFallback = $true })
    [void](Add-Step 'style.paragraph.edit' @{ name = '[Basic Paragraph]'; chars = @{ fontFamily = 'Calibri'; fontStyle = 'Regular'; size = 10.0 } })
    for ($p = 1; $p -le $n; $p++) {
        Say "Reading page $p of $n"
        $place = Page-Place ($p - 1)
        foreach ($sh in (Back-To-Front $doc.Pages.Item($p).Shapes)) { Emit-Shape $sh $place[0] $place[1] }
    }
    # Master page: on each page of PressRoom's master spread.
    if ($doc.MasterPages.Count -ge 1) {
        Say 'Reading the master page'
        $m = $doc.MasterPages.Item(1)
        $parentPages = if ($script:facing) { @(0.0, $pw) } else { @(0.0) }
        foreach ($px in $parentPages) {
            foreach ($sh in (Back-To-Front $m.Shapes)) { Emit-Shape $sh @{ kind = 'parent'; index = 0 } $px }
        }
    }
    # Linked text boxes: thread each story's frames in Publisher's order.
    foreach ($c in $chains) {
        $prev = $c[1]
        $next = $c[0]
        $guard = 0
        while ($next.HasNextLink -eq -1 -and $guard -lt 1000) {
            $guard++
            $next = $next.NextLinkedTextFrame
            $m = $frameStep[[int]$next.Parent.ID]
            if ($null -eq $m) { break }
            [void](Add-Step 'text.thread' @{ from = (Ref $prev 'id'); to = (Ref $m 'id') })
            Emit-Frame-Options $next $m
            $prev = $m
        }
    }
    $doc.Close()
}
finally {
    if ($app) {
        try { $app.Quit() } catch {}
        [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
    }
    Restore-Printer
}

# Build the publication.
Say 'Building the PressRoom publication'
$scriptFile = "$base.import.json"
$json = ConvertTo-Json -InputObject $steps.ToArray() -Depth 12 -Compress
[IO.File]::WriteAllText($scriptFile, $json, $utf8)
$report = & $Cli script $scriptFile --keep-going --save $Out 2>&1 | Out-String
$failed = @()
try {
    $r = $report | ConvertFrom-Json
    for ($i = 0; $i -lt $r.results.Count; $i++) {
        if ($r.results[$i].error) { $failed += "step $i ($($steps[$i].command)): $($r.results[$i].error)" }
    }
} catch { $failed += "could not read the report: $report" }
if ($failed.Count -gt 0) { [IO.File]::WriteAllLines("$base.import-problems.txt", [string[]]$failed, $utf8) }
elseif (-not $KeepWork -and (Test-Path $Out)) {
    # The pictures now live inside the publication.
    Remove-Item $picDir -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item $scriptFile -Force -ErrorAction SilentlyContinue
}
"{0} -> {1}: {2} pages, {3} text boxes ({4} linked boxes), {5} pictures, {6} shapes, {7} lines, {8} tables ({9} cells of mixed formatting simplified), {10} objects kept as pictures, {11} invisible objects left out; {12} steps, {13} problems" -f `
    [IO.Path]::GetFileName($In), [IO.Path]::GetFileName($Out), $n, $stats.text, $stats.linked, $stats.pictures, $stats.shapes, $stats.lines, $stats.tables, $stats.mixedCells, $stats.flattened, $stats.skipped, $steps.Count, $failed.Count
if ($failed.Count -gt 0) { "Problems listed in $base.import-problems.txt" }
