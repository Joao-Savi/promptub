#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Tools = Join-Path $Root "src-tauri\resources\tools"
New-Item -ItemType Directory -Force -Path $Tools | Out-Null

# Remove legado mpv/DLLs — instalador embute so yt-dlp
Get-ChildItem $Tools -File -ErrorAction SilentlyContinue | Where-Object {
    $_.Name -ne "yt-dlp.exe"
} | Remove-Item -Force -ErrorAction SilentlyContinue

$ytdlp = Join-Path $Tools "yt-dlp.exe"
if (-not (Test-Path $ytdlp)) {
    Write-Host "Baixando yt-dlp..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe" -OutFile $ytdlp
}

Write-Host "Ferramentas prontas em src-tauri\resources\tools (somente yt-dlp)" -ForegroundColor Green
