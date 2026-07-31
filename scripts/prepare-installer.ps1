#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Tools = Join-Path $Root "src-tauri\resources\tools"
New-Item -ItemType Directory -Force -Path $Tools | Out-Null

function Get-MpvFromSystem {
    $cmd = Get-Command mpv -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $pf = ${env:ProgramFiles}
    foreach ($p in @("$pf\mpv\mpv.exe", "$pf\MPV Player\mpv.exe")) {
        if (Test-Path $p) { return $p }
    }
    return $null
}

$ytdlp = Join-Path $Tools "yt-dlp.exe"
if (-not (Test-Path $ytdlp)) {
    Write-Host "Baixando yt-dlp..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe" -OutFile $ytdlp
}

$mpvDest = Join-Path $Tools "mpv.exe"
if (-not (Test-Path $mpvDest)) {
    $sysMpv = Get-MpvFromSystem
    if ($sysMpv) {
        Write-Host "Copiando mpv do sistema..." -ForegroundColor Cyan
        Copy-Item $sysMpv $mpvDest -Force
        $mpvDir = Split-Path $sysMpv -Parent
        Get-ChildItem $mpvDir -Filter "*.dll" -ErrorAction SilentlyContinue | ForEach-Object {
            Copy-Item $_.FullName $Tools -Force
        }
    } else {
        Write-Host "Baixando mpv portable..." -ForegroundColor Cyan
        $zip = Join-Path $env:TEMP "promptub-mpv.zip"
        $url = "https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/v0.39.0/mpv-x86_64-v0.39.0.zip"
        Invoke-WebRequest -Uri $url -OutFile $zip
        Expand-Archive -Path $zip -DestinationPath (Join-Path $env:TEMP "promptub-mpv") -Force
        $found = Get-ChildItem (Join-Path $env:TEMP "promptub-mpv") -Recurse -Filter "mpv.exe" | Select-Object -First 1
        if (-not $found) { throw "mpv.exe nao encontrado no pacote baixado" }
        Copy-Item $found.FullName $mpvDest -Force
        Get-ChildItem $found.DirectoryName -Filter "*.dll" | ForEach-Object {
            Copy-Item $_.FullName $Tools -Force
        }
        Remove-Item $zip -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "Ferramentas prontas em src-tauri\resources\tools" -ForegroundColor Green
