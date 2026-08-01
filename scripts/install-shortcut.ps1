#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$ExePath = Join-Path $Root "bin\promptub.exe"
$StampPath = Join-Path $Root "bin\promptub.build.stamp"

$needsBuild = -not (Test-Path $ExePath) -or -not (Test-Path $StampPath)
if ($needsBuild) {
    Write-Host "Gerando build de producao (frontend embutido)..." -ForegroundColor Yellow
    & (Join-Path $PSScriptRoot "build-tauri.ps1")
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

if (-not (Test-Path $ExePath)) {
    Write-Error "promptub.exe nao encontrado. Rode scripts\build-tauri.cmd"
}

$Launcher = Join-Path $PSScriptRoot "open-promptub.cmd"
$Desktop = [Environment]::GetFolderPath("Desktop")
$ShortcutPath = Join-Path $Desktop "promptub.lnk"

$WshShell = New-Object -ComObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = $Launcher
$Shortcut.WorkingDirectory = (Split-Path $Launcher -Parent)
$Shortcut.Description = "YouTube e YouTube Music - estilo Spotify"
$Shortcut.Save()

Write-Host "Atalho criado: $ShortcutPath" -ForegroundColor Green
Write-Host "Use o atalho (nao npm run tauri:dev) para abrir sem terminal."
