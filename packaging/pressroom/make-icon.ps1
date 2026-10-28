# PressRoom's app icon, drawn here (original art): an open booklet, its pages lined, on a rounded
# blue-teal tile. Writes pressroom.ico (16-256 px, PNG entries) and pressroom-256.png beside this
# script. Run with Windows PowerShell: it uses System.Drawing.
param([string]$Out = $PSScriptRoot)
Add-Type -AssemblyName System.Drawing

function Draw-Icon([int]$n) {
    $bmp = New-Object System.Drawing.Bitmap -ArgumentList $n, $n
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)
    $k = $n / 256.0
    # Tile: rounded square, blue-teal top to deep blue bottom.
    $r = 44 * $k
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $w = $n - 1
    $path.AddArc(0, 0, 2 * $r, 2 * $r, 180, 90)
    $path.AddArc($w - 2 * $r, 0, 2 * $r, 2 * $r, 270, 90)
    $path.AddArc($w - 2 * $r, $w - 2 * $r, 2 * $r, 2 * $r, 0, 90)
    $path.AddArc(0, $w - 2 * $r, 2 * $r, 2 * $r, 90, 90)
    $path.CloseFigure()
    $top = [System.Drawing.Color]::FromArgb(255, 18, 132, 150)
    $bot = [System.Drawing.Color]::FromArgb(255, 22, 70, 128)
    $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush -ArgumentList (New-Object System.Drawing.Point 0, 0), (New-Object System.Drawing.Point 0, $n), $top, $bot
    $g.FillPath($brush, $path)
    # Booklet: two pages meeting at a spine, slightly raised at the outer edges.
    $paper = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::White)
    $shade = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 226, 236, 240))
    $cx = 128 * $k
    $left = [System.Drawing.PointF[]]@(
        (New-Object System.Drawing.PointF (40 * $k), (70 * $k)),
        (New-Object System.Drawing.PointF ($cx - 3 * $k), (84 * $k)),
        (New-Object System.Drawing.PointF ($cx - 3 * $k), (206 * $k)),
        (New-Object System.Drawing.PointF (40 * $k), (192 * $k)))
    $right = [System.Drawing.PointF[]]@(
        (New-Object System.Drawing.PointF ($cx + 3 * $k), (84 * $k)),
        (New-Object System.Drawing.PointF (216 * $k), (70 * $k)),
        (New-Object System.Drawing.PointF (216 * $k), (192 * $k)),
        (New-Object System.Drawing.PointF ($cx + 3 * $k), (206 * $k)))
    $g.FillPolygon($shade, $left)
    $g.FillPolygon($paper, $right)
    # Lines of text on both pages; a gold heading bar on the right page.
    if ($n -ge 32) {
        $ink = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 120, 150, 170)), ([Math]::Max(1.0, 7 * $k))
        foreach ($i in 0..4) {
            $y = 108 + $i * 18
            $g.DrawLine($ink, (54 * $k), (($y - 8) * $k), (($cx / $k - 16) * $k), ($y * $k))
            $g.DrawLine($ink, (($cx / $k + 16) * $k), (($y + 10) * $k), (202 * $k), (($y + 2) * $k))
        }
        $gold = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 238, 176, 58))
        $g.FillPolygon($gold, [System.Drawing.PointF[]]@(
            (New-Object System.Drawing.PointF ($cx + 16 * $k), (96 * $k)),
            (New-Object System.Drawing.PointF (202 * $k), (88 * $k)),
            (New-Object System.Drawing.PointF (202 * $k), (104 * $k)),
            (New-Object System.Drawing.PointF ($cx + 16 * $k), (112 * $k))))
    }
    $g.Dispose()
    return $bmp
}

$sizes = 16, 24, 32, 48, 64, 128, 256
$pngs = @()
foreach ($s in $sizes) {
    $b = Draw-Icon $s
    $ms = New-Object System.IO.MemoryStream
    $b.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    if ($s -eq 256) { $b.Save((Join-Path $Out 'pressroom-256.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
    $b.Dispose()
    $pngs += , $ms.ToArray()
}
# ICO: header, one 16-byte entry per size, then the PNG images.
$fs = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter $fs
$bw.Write([UInt16]0); $bw.Write([UInt16]1); $bw.Write([UInt16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $s = $sizes[$i]
    $dim = if ($s -ge 256) { 0 } else { $s }
    $bw.Write([byte]$dim); $bw.Write([byte]$dim); $bw.Write([byte]0); $bw.Write([byte]0)
    $bw.Write([UInt16]1); $bw.Write([UInt16]32)
    $bw.Write([UInt32]$pngs[$i].Length); $bw.Write([UInt32]$offset)
    $offset += $pngs[$i].Length
}
foreach ($p in $pngs) { $bw.Write($p) }
$bw.Flush()
[System.IO.File]::WriteAllBytes((Join-Path $Out 'pressroom.ico'), $fs.ToArray())
"wrote pressroom.ico and pressroom-256.png"
