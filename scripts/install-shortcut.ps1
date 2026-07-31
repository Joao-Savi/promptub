#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$ExePath = Join-Path $Root "bin\promptub.exe"

if (-not (Test-Path $ExePath)) {
    Write-Host "Compilando promptub..." -ForegroundColor Yellow
    & (Join-Path $PSScriptRoot "build-tauri.ps1")
}

if (-not (Test-Path $ExePath)) {
    Write-Error "promptub.exe nao encontrado. Instale Visual C++ Build Tools e rode scripts\build-tauri.ps1"
}

$Desktop = [Environment]::GetFolderPath("Desktop")
$ShortcutPath = Join-Path $Desktop "promptub.lnk"

$WshShell = New-Object -ComObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = $ExePath
$Shortcut.WorkingDirectory = (Split-Path $ExePath -Parent)
$Shortcut.Description = "YouTube e YouTube Music - estilo Spotify"
$Shortcut.Save()

Write-Host "Atalho criado: $ShortcutPath" -ForegroundColor Green
Write-Host "Use o atalho (nao npm run tauri:dev) para abrir sem terminal."
