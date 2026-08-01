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
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/shinchiro/mpv-winbuild-cmake/releases/latest"
        $asset = $release.assets | Where-Object { $_.name -match '^mpv-x86_64-\d{8}-git-.+\.7z$' -and $_.name -notmatch '-v3-' } | Select-Object -First 1
        if (-not $asset) { throw "Nenhum asset mpv-x86_64 encontrado no release latest" }
        $archive = Join-Path $env:TEMP "promptub-mpv.7z"
        $extract = Join-Path $env:TEMP "promptub-mpv"
        Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $archive
        if (Test-Path $extract) { Remove-Item $extract -Recurse -Force }
        New-Item -ItemType Directory -Force -Path $extract | Out-Null
        $sevenZip = @(
            "${env:ProgramFiles}\7-Zip\7z.exe",
            "${env:ProgramFiles(x86)}\7-Zip\7z.exe"
        ) | Where-Object { Test-Path $_ } | Select-Object -First 1
        if ($sevenZip) {
            & $sevenZip x $archive "-o$extract" -y | Out-Null
        } elseif (Get-Command tar -ErrorAction SilentlyContinue) {
            tar -xf $archive -C $extract
        } else {
            throw "Baixe o mpv ou instale o 7-Zip para extrair o pacote .7z"
        }
        $found = Get-ChildItem $extract -Recurse -Filter "mpv.exe" | Select-Object -First 1
        if (-not $found) { throw "mpv.exe nao encontrado no pacote baixado" }
        Copy-Item $found.FullName $mpvDest -Force
        Get-ChildItem $found.DirectoryName -Filter "*.dll" | ForEach-Object {
            Copy-Item $_.FullName $Tools -Force
        }
        Remove-Item $archive -Force -ErrorAction SilentlyContinue
        Remove-Item $extract -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "Ferramentas prontas em src-tauri\resources\tools" -ForegroundColor Green
