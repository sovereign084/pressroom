<#
.SYNOPSIS
Convert a Microsoft Publisher file (.pub) into a PressRoom publication (.pressroom).

.DESCRIPTION
Runs on a Windows PC that has Publisher installed. Publisher itself opens the .pub (read-only) and
reports every page, text box, picture and shape; this script rebuilds them with PressRoom's
command-line tool:
  - text boxes keep their text, fonts, sizes, bold/italic/underline, colours, paragraph alignment and
    spacing, inner margins, columns and vertical alignment; linked text boxes stay linked;
  - pictures are saved at 300 dpi and placed in picture boxes of the same size and place;
  - plain rectangles and ovals stay editable shapes with their fill and outline;
  - anything else (WordArt, tables, lines, fancy shapes, groups' odd members) is saved by Publisher as
    a picture, so it looks the same even though it is no longer editable;
  - the master page's objects go on PressRoom's master page.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File pub2pressroom.ps1 -In "Bulletin Oct 26.pub"
Writes "Bulletin Oct 26.pressroom" beside it, and a "Bulletin Oct 26 pictures" folder.
#>
param(
    [Parameter(Mandatory)][string]$In,
    [string]$Out,
    [string]$Cli = (Join-Path $PSScriptRoot 'PressRoom-cli.exe'),
    # Also export Publisher's own PDF of the file, to compare against.
    [switch]$Reference
)
$ErrorActionPreference = 'Stop'
$In = (Resolve-Path $In).Path
# A folder: every .pub in it (and below), each beside its original.
if (Test-Path $In -PathType Container) {
    $files = Get-ChildItem $In -Recurse -Filter *.pub | Where-Object { -not $_.PSIsContainer }
    foreach ($file in $files) {
        try { & $PSCommandPath -In $file.FullName -Cli $Cli -Reference:$Reference } catch { "{0}: FAILED - {1}" -f $file.Name, $_.Exception.Message }
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

# Character formatting of a range, or $null when it is mixed.
function Read-Font($range) {
    $f = $range.Font
    $name = $f.Name; $size = $f.Size; $b = $f.Bold; $i = $f.Italic; $u = $f.Underline
    if (-not $name -or $size -le 0 -or $b -eq -2 -or $i -eq -2 -or ($u -ne 0 -and $u -ne 1 -and $u -ne -1)) { return $null }
    return [pscustomobject]@{ Name = $name; Size = [double]$size; Bold = ($b -eq -1); Italic = ($i -eq -1); Underline = ($u -ne 0); Color = [int]$f.Color.RGB }
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

# A font's natural line height as a multiple of its size (single spacing in Publisher).
function Line-Factor([string]$name) {
    switch -Wildcard ($name) { 'Calibri*' { 1.22 } 'Cambria*' { 1.17 } 'Times New Roman' { 1.15 } 'Arial*' { 1.15 } 'Georgia' { 1.14 } 'Garamond' { 1.12 } 'Verdana' { 1.22 } 'Segoe*' { 1.33 } default { 1.2 } }
}

function Same-Font($a, $b) {
    return $a.Name -eq $b.Name -and $a.Size -eq $b.Size -and $a.Bold -eq $b.Bold -and $a.Italic -eq $b.Italic -and $a.Underline -eq $b.Underline -and $a.Color -eq $b.Color
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
$shapeAt = @{}       # Publisher shape ID → @(spread, x) for linked boxes found later
$picN = 0
$stats = @{ text = 0; pictures = 0; shapes = 0; flattened = 0; linked = 0 }

function Emit-Rotation($shape, [int]$step) {
    $rot = [double]$shape.Rotation
    if ($rot -ne 0) { [void](Add-Step 'transform.rotate' @{ angle = -$rot; ids = @((Ref $step 'id')) }) }
}

function Emit-Picture($shape, $spread, [double]$x) {
    $script:picN++
    $file = Join-Path $picDir ('picture-{0:000}.png' -f $script:picN)
    try { $shape.SaveAsPicture($file, 'pbPictureResolutionCommercialPrint_300dpi') } catch { $shape.SaveAsPicture($file) }
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
    [void](Add-Step 'file.place' @{ path = $file; frame = (Ref $k 'id') })
    [void](Add-Step 'object.fit' @{ mode = 'fitContentToFrame'; ids = @((Ref $k 'id')) })
    [void](Add-Step 'object.stroke' @{ swatch = '[None]'; ids = @((Ref $k 'id')) })
    return $k
}

function Emit-Box-Style($shape, [int]$k) {
    try {
        if ($shape.Fill.Visible -eq -1) { [void](Add-Step 'object.color' @{ color = (Hex $shape.Fill.ForeColor.RGB); target = 'fill'; ids = @((Ref $k 'id')) }) }
        if ($shape.Line.Visible -eq -1) {
            [void](Add-Step 'object.stroke' @{ weight = [double]$shape.Line.Weight; ids = @((Ref $k 'id')) })
            [void](Add-Step 'object.color' @{ color = (Hex $shape.Line.ForeColor.RGB); target = 'stroke'; ids = @((Ref $k 'id')) })
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

function Emit-Text($shape, $spread, [double]$x) {
    $tf = $shape.TextFrame
    if ($tf.HasPreviousLink -eq -1) { return }   # Its text arrives through the link.
    $range = if ($tf.HasNextLink -eq -1) { $tf.Story.TextRange } else { $tf.TextRange }
    $raw = [string]$range.Text
    $trim = $raw.TrimEnd("`r")
    $text = Convert-Text $trim
    $k = Add-Step 'frame.create' @{ spread = $spread; rect = (Rect $shape $x); content = 'text'; text = $text }
    $frameStep[[int]$shape.ID] = $k
    $script:stats.text++
    Emit-Frame-Options $tf $k
    Emit-Box-Style $shape $k
    Emit-Rotation $shape $k
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
        $attrs = @{ align = $alignMap[[int]$pf.Alignment] }
        if (-not $attrs.align) { $attrs.align = 'left' }
        try { $attrs.spaceBefore = [double]$pf.SpaceBefore; $attrs.spaceAfter = [double]$pf.SpaceAfter } catch {}
        try { $attrs.leftIndent = [double]$pf.LeftIndent; $attrs.firstLineIndent = [double]$pf.FirstLineIndent } catch {}
        # Line spacing: Publisher's 'lines' (1.19 by default) or exact points.
        $lines = 1.0; $exact = 0.0
        try {
            switch ([int]$pf.LineSpacingRule) { 1 { $lines = 1.5 } 2 { $lines = 2.0 } 4 { $exact = [double]$pf.LineSpacing } 5 { $lines = [double]$pf.LineSpacing } }
        } catch {}
        [void](Add-Step 'text.select' @{ story = (Ref $k 'story'); anchor = $a; focus = $b })
        [void](Add-Step 'type.para' @{ attrs = $attrs })
        # Character runs, merged where the formatting repeats.
        $runs = Get-Runs $range $ps ([Math]::Max(1, $pl))
        $merged = New-Object System.Collections.Generic.List[object]
        foreach ($r in $runs) {
            if ($merged.Count -gt 0) {
                $last = $merged[$merged.Count - 1]
                if ((Same-Font $last[2] $r[2]) -and ($last[0] + $last[1] -eq $r[0])) { $merged[$merged.Count - 1] = @($last[0], ($last[1] + $r[1]), $last[2]); continue }
            }
            $merged.Add($r)
        }
        foreach ($r in $merged) {
            $ra = ByteOffset $text $r[0]; $rb = ByteOffset $text ([Math]::Min($text.Length, $r[0] + $r[1]))
            if ($rb -le $ra) { continue }
            $f = $r[2]
            [void](Add-Step 'text.select' @{ story = (Ref $k 'story'); anchor = $ra; focus = $rb })
            $lead = if ($exact -gt 0) { $exact } else { $f.Size * (Line-Factor $f.Name) * $lines }
            [void](Add-Step 'type.char' @{ attrs = @{ fontFamily = $f.Name; size = $f.Size; underline = $f.Underline; leading = @{ kind = 'points'; value = [Math]::Round($lead, 2) } } })
            [void](Add-Step 'type.char' @{ attrs = @{ fontStyle = (Style-Name $f.Bold $f.Italic) } })
            if ($f.Color -ne 0) { [void](Add-Step 'object.color' @{ color = (Hex $f.Color); text = $true }) }
        }
    }
    [void](Add-Step 'edit.deselectAll' @{})
    # The rest of a linked chain: empty boxes threaded after this one.
    $prevStep = $k
    $next = $tf
    while ($next.HasNextLink -eq -1) {
        $next = $next.NextLinkedTextFrame
        $ns = $next.Parent
        $place = $shapeAt[[int]$ns.ID]
        if (-not $place) { break }
        $m = Add-Step 'frame.create' @{ spread = $place[0]; rect = (Rect $ns $place[1]); content = 'unassigned' }
        [void](Add-Step 'text.thread' @{ from = (Ref $prevStep 'id'); to = (Ref $m 'id') })
        [void](Add-Step 'object.stroke' @{ swatch = '[None]'; ids = @((Ref $m 'id')) })
        Emit-Frame-Options $next $m
        $script:stats.linked++
        $prevStep = $m
    }
}

function Emit-Shape($shape, $spread, [double]$x) {
    $type = [int]$shape.Type
    if ($type -eq 6) {   # Group: its members, each in its own place.
        foreach ($i in 1..$shape.GroupItems.Count) { Emit-Shape $shape.GroupItems.Item($i) $spread $x }
        return
    }
    $hasText = $false
    try { $hasText = ($shape.HasTextFrame -eq -1) -and ($type -eq 17 -or [int]$shape.TextFrame.Story.TextRange.Length -gt 1) } catch {}
    if ($hasText) { Emit-Text $shape $spread $x; return }
    if ($type -eq 1 -and @(1, 9) -contains [int]$shape.AutoShapeType) {
        $kind = if ([int]$shape.AutoShapeType -eq 9) { 'ellipse' } else { 'rectangle' }
        $k = Add-Step 'frame.create' @{ spread = $spread; rect = (Rect $shape $x); shape = $kind; content = 'unassigned' }
        Emit-Box-Style $shape $k
        Emit-Rotation $shape $k
        $script:stats.shapes++
        return
    }
    # Pictures (embedded 13, linked 11, placeholders 14); anything else is kept as its picture.
    if (@(11, 13, 14) -contains $type) { $script:stats.pictures++ } else { $script:stats.flattened++ }
    [void](Emit-Picture $shape $spread $x)
}

$app = New-Object -ComObject Publisher.Application
try {
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
    [void](Add-Step 'style.paragraph.edit' @{ name = '[Basic Paragraph]'; chars = @{ fontFamily = 'Calibri'; fontStyle = 'Regular'; size = 10.0 } })
    # Where every shape is, first: linked boxes may sit on a later page.
    for ($p = 1; $p -le $n; $p++) {
        $place = Page-Place ($p - 1)
        $pg = $doc.Pages.Item($p)
        for ($s = 1; $s -le $pg.Shapes.Count; $s++) { $shapeAt[[int]$pg.Shapes.Item($s).ID] = $place }
    }
    for ($p = 1; $p -le $n; $p++) {
        $place = Page-Place ($p - 1)
        $pg = $doc.Pages.Item($p)
        for ($s = 1; $s -le $pg.Shapes.Count; $s++) { Emit-Shape $pg.Shapes.Item($s) $place[0] $place[1] }
    }
    # Master page: on each page of PressRoom's master spread.
    if ($doc.MasterPages.Count -ge 1) {
        $m = $doc.MasterPages.Item(1)
        $parentPages = if ($script:facing) { @(0.0, $pw) } else { @(0.0) }
        foreach ($px in $parentPages) {
            for ($s = 1; $s -le $m.Shapes.Count; $s++) { Emit-Shape $m.Shapes.Item($s) @{ kind = 'parent'; index = 0 } $px }
        }
    }
    if ($Reference) {
        try { $doc.ExportAsFixedFormat(2, "$base (Publisher).pdf") } catch { Write-Warning "Publisher's own PDF could not be made: $($_.Exception.Message)" }
    }
    $doc.Close()
}
finally {
    $app.Quit()
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}

# Build the publication.
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
"{0} -> {1}: {2} pages, {3} text boxes ({4} linked boxes), {5} pictures, {6} shapes, {7} objects kept as pictures; {8} steps, {9} problems" -f `
    [IO.Path]::GetFileName($In), [IO.Path]::GetFileName($Out), $n, $stats.text, $stats.linked, $stats.pictures, $stats.shapes, $stats.flattened, $steps.Count, $failed.Count
if ($failed.Count -gt 0) { "Problems listed in $base.import-problems.txt" }
