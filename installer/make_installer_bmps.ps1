# Regenerates installer\dialog.bmp and installer\banner.bmp — the
# WixUIDialogBmp/WixUIBannerBmp artwork referenced from main.wxs — from the
# same hero screenshot used on the splash site. Not run automatically by any
# build; re-run by hand (`powershell -File installer\make_installer_bmps.ps1`
# from the repo root) only when that source image or the desired crop/
# wordmark changes. Needs network access once, to fetch the DotGothic16 font
# file used for the banner wordmark (the site loads it from Google Fonts too;
# not vendored here to avoid checking in a ~1.9MB font binary for a build
# script that only needs it transiently).

Add-Type -AssemblyName System.Drawing

$repoRoot = Split-Path -Parent $PSScriptRoot
$srcPath = Join-Path $repoRoot "site\src\assets\screensaver-hero.png"
$fontPath = Join-Path $env:TEMP "neo_win_pipes-DotGothic16-Regular.ttf"

if (-not (Test-Path $fontPath)) {
    Invoke-WebRequest -Uri "https://fonts.gstatic.com/s/dotgothic16/v21/v6-QGYjBJFKgyw5nSoDAGE7L.ttf" -OutFile $fontPath
}

$src = [System.Drawing.Image]::FromFile($srcPath)

$fontCollection = New-Object System.Drawing.Text.PrivateFontCollection
$fontCollection.AddFontFile($fontPath)
$fontFamily = $fontCollection.Families[0]

function New-CroppedResizedBmp {
    param(
        [System.Drawing.Image]$Source,
        [double]$FocusX,   # 0..1, horizontal center of the crop within the source
        [double]$FocusY,   # 0..1, vertical center of the crop within the source
        [int]$TargetW,
        [int]$TargetH,
        [string]$OutPath,
        [switch]$DarkenForText,
        [string]$WordmarkText,
        [float]$WordmarkSize,
        [string]$WordmarkAnchor  # "bottom-left" or "top-left"
    )
    $srcAspect = $Source.Width / [double]$Source.Height
    $dstAspect = $TargetW / [double]$TargetH

    if ($dstAspect -gt $srcAspect) {
        $cropW = $Source.Width
        $cropH = [int]([double]$Source.Width / $dstAspect)
    } else {
        $cropH = $Source.Height
        $cropW = [int]([double]$Source.Height * $dstAspect)
    }

    $cx = [int]($Source.Width * $FocusX)
    $cy = [int]($Source.Height * $FocusY)
    $cropX = [Math]::Max(0, [Math]::Min($Source.Width - $cropW, $cx - [int]($cropW / 2)))
    $cropY = [Math]::Max(0, [Math]::Min($Source.Height - $cropH, $cy - [int]($cropH / 2)))

    # MSI dialog bitmaps must be plain 24bpp BMPs, not 32bpp-with-alpha.
    $bmp = New-Object System.Drawing.Bitmap $TargetW, $TargetH, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAlias
    $srcRect = New-Object System.Drawing.Rectangle $cropX, $cropY, $cropW, $cropH
    $dstRect = New-Object System.Drawing.Rectangle 0, 0, $TargetW, $TargetH
    $g.DrawImage($Source, $dstRect, $srcRect, [System.Drawing.GraphicsUnit]::Pixel)

    if ($DarkenForText) {
        $overlayBrush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(110, 11, 13, 18))
        $g.FillRectangle($overlayBrush, $dstRect)
        $overlayBrush.Dispose()
    }

    if ($WordmarkText) {
        $font = New-Object System.Drawing.Font $fontFamily, $WordmarkSize, ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)
        $textBrush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 255, 255, 255))
        $shadowBrush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(160, 0, 0, 0))
        $size = $g.MeasureString($WordmarkText, $font)
        $pad = [Math]::Max(6, [int]($TargetH * 0.12))
        if ($WordmarkAnchor -eq "bottom-left") {
            $x = $pad
            $y = $TargetH - $size.Height - $pad
        } else {
            $x = $pad
            $y = $pad
        }
        $g.DrawString($WordmarkText, $font, $shadowBrush, $x + 1.5, $y + 1.5)
        $g.DrawString($WordmarkText, $font, $textBrush, $x, $y)
        $font.Dispose()
        $textBrush.Dispose()
        $shadowBrush.Dispose()
    }

    $g.Dispose()
    $bmp.Save($OutPath, [System.Drawing.Imaging.ImageFormat]::Bmp)
    $bmp.Dispose()
}

# Dialog graphic: full WelcomeEulaDlg background, 493x312. No wordmark here —
# WixUI_Minimal stacks its own title text and a license scroll box on top of
# nearly this whole bitmap, so custom text baked into the image would clash
# with or hide behind those controls.
New-CroppedResizedBmp -Source $src -FocusX 0.32 -FocusY 0.62 -TargetW 493 -TargetH 312 `
    -OutPath (Join-Path $repoRoot "installer\dialog.bmp")

# Banner graphic: ProgressDlg's top strip, 493x58 — fully clear of other
# controls, so this is where the real "neo_win_pipes" wordmark (rendered in
# the site's own DotGothic16 heading font) actually shows up as designed.
New-CroppedResizedBmp -Source $src -FocusX 0.55 -FocusY 0.22 -TargetW 493 -TargetH 58 `
    -OutPath (Join-Path $repoRoot "installer\banner.bmp") `
    -DarkenForText -WordmarkText "neo_win_pipes" -WordmarkSize 24 -WordmarkAnchor "top-left"

$src.Dispose()
Write-Host "Wrote installer\dialog.bmp and installer\banner.bmp"
