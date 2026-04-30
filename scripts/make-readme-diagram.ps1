Add-Type -AssemblyName System.Drawing

$W = 1200
$H = 520
$bmp = New-Object System.Drawing.Bitmap($W, $H)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAlias

$bg       = [System.Drawing.Color]::FromArgb(255, 17, 24, 39)
$dotColor = [System.Drawing.Color]::FromArgb(255, 31, 41, 55)
$cardFill = [System.Drawing.Color]::FromArgb(255, 31, 41, 55)
$cardEdge = [System.Drawing.Color]::FromArgb(255, 75, 85, 99)
$accent   = [System.Drawing.Color]::FromArgb(255, 96, 165, 250)
$accent2  = [System.Drawing.Color]::FromArgb(255, 167, 139, 250)
$accent3  = [System.Drawing.Color]::FromArgb(255, 52, 211, 153)
$accent4  = [System.Drawing.Color]::FromArgb(255, 251, 191, 36)
$white    = [System.Drawing.Color]::White

$g.Clear($bg)

$dotBrush = New-Object System.Drawing.SolidBrush($dotColor)
for ($x = 20; $x -lt $W; $x += 28) {
    for ($y = 20; $y -lt $H; $y += 28) {
        $g.FillEllipse($dotBrush, [int]$x, [int]$y, 2, 2)
    }
}

function Draw-Card {
    param([int]$x, [int]$y, [int]$w, [int]$h, $accentColor)
    $shadow = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(80, 0, 0, 0))
    $g.FillRectangle($shadow, ($x + 4), ($y + 6), $w, $h)
    $fill = New-Object System.Drawing.SolidBrush ($cardFill)
    $g.FillRectangle($fill, $x, $y, $w, $h)
    $stripBrush = New-Object System.Drawing.SolidBrush ($accentColor)
    $g.FillRectangle($stripBrush, $x, $y, $w, 4)
    $pen = New-Object System.Drawing.Pen ($cardEdge, 1.5)
    $g.DrawRectangle($pen, $x, $y, $w, $h)
}

function Draw-Handle {
    param([int]$cx, [int]$cy, $color)
    $r = 7
    $b = New-Object System.Drawing.SolidBrush ($color)
    $g.FillEllipse($b, ($cx - $r), ($cy - $r), ($r * 2), ($r * 2))
    $p = New-Object System.Drawing.Pen ($white, 2)
    $g.DrawEllipse($p, ($cx - $r), ($cy - $r), ($r * 2), ($r * 2))
}

function Draw-Edge {
    param([int]$x1, [int]$y1, [int]$x2, [int]$y2, $color)
    $pen = New-Object System.Drawing.Pen ($color, 3)
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $dx = [int][Math]::Max(60, ($x2 - $x1) / 2)
    $cp1 = New-Object System.Drawing.PointF (($x1 + $dx), $y1)
    $cp2 = New-Object System.Drawing.PointF (($x2 - $dx), $y2)
    $p1 = New-Object System.Drawing.PointF ($x1, $y1)
    $p2 = New-Object System.Drawing.PointF ($x2, $y2)
    $g.DrawBezier($pen, $p1, $cp1, $cp2, $p2)
}

function Draw-MicIcon {
    param([int]$cx, [int]$cy)
    $brush = New-Object System.Drawing.SolidBrush ($white)
    $pen = New-Object System.Drawing.Pen ($white, 3)
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $g.FillEllipse($brush, ($cx - 12), ($cy - 26), 24, 36)
    $g.DrawArc($pen, ($cx - 20), ($cy - 12), 40, 30, 20, 140)
    $g.DrawLine($pen, $cx, ($cy + 18), $cx, ($cy + 26))
    $g.DrawLine($pen, ($cx - 12), ($cy + 26), ($cx + 12), ($cy + 26))
}

function Draw-SpeakerIcon {
    param([int]$cx, [int]$cy)
    $brush = New-Object System.Drawing.SolidBrush ($white)
    $pen = New-Object System.Drawing.Pen ($white, 3)
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pts = [System.Drawing.Point[]]@(
        (New-Object System.Drawing.Point (($cx - 18), ($cy - 8))),
        (New-Object System.Drawing.Point (($cx - 6),  ($cy - 8))),
        (New-Object System.Drawing.Point (($cx + 8),  ($cy - 22))),
        (New-Object System.Drawing.Point (($cx + 8),  ($cy + 22))),
        (New-Object System.Drawing.Point (($cx - 6),  ($cy + 8))),
        (New-Object System.Drawing.Point (($cx - 18), ($cy + 8)))
    )
    $g.FillPolygon($brush, $pts)
    $g.DrawArc($pen, ($cx + 8), ($cy - 14), 14, 28, -60, 120)
    $g.DrawArc($pen, ($cx + 14), ($cy - 22), 22, 44, -60, 120)
}

function Draw-MixerIcon {
    param([int]$cx, [int]$cy)
    $pen = New-Object System.Drawing.Pen ($white, 3)
    $brush = New-Object System.Drawing.SolidBrush ($white)
    $offsets = @(-18, 0, 18)
    $knobYs  = @(($cy + 8), ($cy - 6), ($cy + 14))
    for ($i = 0; $i -lt 3; $i++) {
        $sx = $cx + $offsets[$i]
        $g.DrawLine($pen, [int]$sx, ($cy - 22), [int]$sx, ($cy + 22))
        $g.FillRectangle($brush, ([int]$sx - 8), ([int]$knobYs[$i] - 4), 16, 8)
    }
}

function Draw-DriverIcon {
    param([int]$cx, [int]$cy)
    $pen = New-Object System.Drawing.Pen ($white, 3)
    $brush = New-Object System.Drawing.SolidBrush ($white)
    $g.DrawRectangle($pen, ($cx - 18), ($cy - 18), 36, 36)
    $g.FillRectangle($brush, ($cx - 8), ($cy - 8), 16, 16)
    foreach ($i in @(-10, 0, 10)) {
        $g.DrawLine($pen, ($cx + $i), ($cy - 24), ($cx + $i), ($cy - 18))
        $g.DrawLine($pen, ($cx + $i), ($cy + 18), ($cx + $i), ($cy + 24))
        $g.DrawLine($pen, ($cx - 24), ($cy + $i), ($cx - 18), ($cy + $i))
        $g.DrawLine($pen, ($cx + 18), ($cy + $i), ($cx + 24), ($cy + $i))
    }
}

function Draw-WaveIcon {
    param([int]$cx, [int]$cy)
    $pen = New-Object System.Drawing.Pen ($white, 3)
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pts = New-Object System.Collections.Generic.List[System.Drawing.PointF]
    for ($i = 0; $i -le 60; $i++) {
        $t = $i / 60.0
        $px = $cx - 30 + $i
        $py = $cy + [Math]::Sin($t * [Math]::PI * 3) * 16
        $pts.Add((New-Object System.Drawing.PointF ([single]$px, [single]$py)))
    }
    $g.DrawLines($pen, $pts.ToArray())
}

$cardW = 170
$cardH = 110

$x1 = 80;   $y1 = 80
Draw-Card $x1 $y1 $cardW $cardH $accent
Draw-MicIcon ($x1 + $cardW/2) ($y1 + $cardH/2 + 4)
Draw-Handle ($x1 + $cardW) ($y1 + $cardH/2) $accent

$x2 = 80;   $y2 = 320
Draw-Card $x2 $y2 $cardW $cardH $accent2
Draw-DriverIcon ($x2 + $cardW/2) ($y2 + $cardH/2 + 4)
Draw-Handle ($x2 + $cardW) ($y2 + $cardH/2) $accent2

$x3 = 500;  $y3 = 200
Draw-Card $x3 $y3 $cardW $cardH $accent4
Draw-MixerIcon ($x3 + $cardW/2) ($y3 + $cardH/2 + 4)
Draw-Handle $x3 ($y3 + $cardH/2 - 22) $accent
Draw-Handle $x3 ($y3 + $cardH/2 + 22) $accent2
Draw-Handle ($x3 + $cardW) ($y3 + $cardH/2) $accent4

$x4 = 940;  $y4 = 80
Draw-Card $x4 $y4 $cardW $cardH $accent3
Draw-SpeakerIcon ($x4 + $cardW/2 - 12) ($y4 + $cardH/2)
Draw-Handle $x4 ($y4 + $cardH/2) $accent3

$x5 = 940;  $y5 = 320
Draw-Card $x5 $y5 $cardW $cardH $accent
Draw-WaveIcon ($x5 + $cardW/2) ($y5 + $cardH/2 + 4)
Draw-Handle $x5 ($y5 + $cardH/2) $accent

Draw-Edge ($x1 + $cardW) ($y1 + $cardH/2) $x3 ($y3 + $cardH/2 - 22) $accent
Draw-Edge ($x2 + $cardW) ($y2 + $cardH/2) $x3 ($y3 + $cardH/2 + 22) $accent2
Draw-Edge ($x3 + $cardW) ($y3 + $cardH/2) $x4 ($y4 + $cardH/2) $accent4
Draw-Edge ($x3 + $cardW) ($y3 + $cardH/2) $x5 ($y5 + $cardH/2) $accent4

$out = "C:\Users\nwh63\cable\docs\images\readme-diagram.jpg"
$null = New-Item -ItemType Directory -Force -Path (Split-Path $out)
$encoder = [System.Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object { $_.MimeType -eq "image/jpeg" }
$params = New-Object System.Drawing.Imaging.EncoderParameters(1)
$params.Param[0] = New-Object System.Drawing.Imaging.EncoderParameter ([System.Drawing.Imaging.Encoder]::Quality, [int64]92)
$bmp.Save($out, $encoder, $params)
$g.Dispose(); $bmp.Dispose()
Write-Host "Saved: $out"
