#Requires -Version 5.1
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$Root = Split-Path -Parent $PSScriptRoot
$Out = Join-Path $Root "src-tauri\icons\icon-source.png"
$Size = 512

$bmp = New-Object System.Drawing.Bitmap $Size, $Size
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit

$bg = [System.Drawing.Color]::FromArgb(255, 12, 8, 8)
$g.Clear($bg)

$borderPen = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 255, 34, 34)), 6
$g.DrawRectangle($borderPen, 24, 24, $Size - 48, $Size - 48)

$accent = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 255, 34, 34))
$font = New-Object System.Drawing.Font("Consolas", 148, [System.Drawing.FontStyle]::Bold)
$g.DrawString(">", $font, $accent, 150, 148)

$fontSmall = New-Object System.Drawing.Font("Consolas", 52, [System.Drawing.FontStyle]::Bold)
$g.DrawString("_", $fontSmall, $accent, 318, 318)

$accent.Dispose()

$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()

Write-Host "Icone fonte: $Out" -ForegroundColor Green

Push-Location $Root
try {
    npm run tauri -- icon $Out
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Write-Host "Icones Tauri gerados em src-tauri\icons\" -ForegroundColor Green
} finally {
    Pop-Location
}
