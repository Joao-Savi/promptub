#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$env:CARGO_TARGET_DIR = Join-Path $Root "src-tauri\target"

. (Join-Path $PSScriptRoot "setup-vs.ps1")

Write-Host "Preparando ferramentas (yt-dlp + mpv)..." -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "prepare-installer.ps1")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Building promptub (Tauri)..." -ForegroundColor Cyan
Push-Location $Root
try {
    npm run tauri -- build
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $exe = Join-Path $Root "src-tauri\target\release\promptub.exe"
    if (-not (Test-Path $exe)) {
        $fallback = Get-ChildItem -Path $env:TEMP -Recurse -Filter "promptub.exe" -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match "cargo-target\\release\\promptub.exe" } |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($fallback) {
            New-Item -ItemType Directory -Force -Path (Split-Path $exe -Parent) | Out-Null
            Copy-Item $fallback.FullName $exe -Force
        }
    }
    if (-not (Test-Path $exe)) {
        throw "Build finished but exe not found at $exe"
    }

    $bin = Join-Path $Root "bin"
    New-Item -ItemType Directory -Force -Path $bin | Out-Null
    Copy-Item $exe (Join-Path $bin "promptub.exe") -Force
    Set-Content -Path (Join-Path $bin "promptub.build.stamp") -Value "tauri-build" -Encoding ascii

    $releaseTools = Join-Path $Root "src-tauri\target\release\tools"
    $srcTools = Join-Path $Root "src-tauri\resources\tools"
    $binTools = Join-Path $bin "tools"
    if (Test-Path $releaseTools) {
        Copy-Item $releaseTools $binTools -Recurse -Force
    } elseif (Test-Path $srcTools) {
        Copy-Item $srcTools $binTools -Recurse -Force
    }
    Write-Host "Tools: bin\tools" -ForegroundColor Green

    $setupDir = Join-Path $Root "src-tauri\target\release\bundle\nsis"
    $setup = Get-ChildItem $setupDir -Filter "*setup.exe" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if (-not $setup) {
        $setup = Get-ChildItem -Path $env:TEMP -Recurse -Filter "promptub_*setup.exe" -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
    }
    if ($setup) {
        Get-ChildItem $bin -Filter "*setup.exe" -ErrorAction SilentlyContinue | Remove-Item -Force
        Copy-Item $setup.FullName (Join-Path $bin $setup.Name) -Force
        Write-Host "Installer: bin\$($setup.Name)" -ForegroundColor Green
    }

    Write-Host "Ready: bin\promptub.exe" -ForegroundColor Green
}
finally {
    Pop-Location
}
