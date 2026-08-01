#Requires -Version 5.1
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$Exe = @(
    Join-Path $env:LOCALAPPDATA "Programs\promptub\promptub.exe"
    Join-Path $Root "bin\promptub.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $Exe) {
    Write-Host "promptub.exe nao encontrado." -ForegroundColor Red
    Write-Host "Instale pelo setup ou rode scripts\build-tauri.ps1" -ForegroundColor Yellow
    Read-Host "Enter para sair"
    exit 1
}

$installDir = Split-Path $Exe -Parent
Get-ChildItem $installDir -Recurse -Include *.exe,*.dll -ErrorAction SilentlyContinue | ForEach-Object {
    Unblock-File -LiteralPath $_.FullName -ErrorAction SilentlyContinue
}

try {
    Start-Process -FilePath $Exe -WorkingDirectory $installDir -ErrorAction Stop
    Write-Host "promptub iniciado." -ForegroundColor Green
} catch {
    Write-Host "Controle de Aplicativo bloqueou o promptub.exe." -ForegroundColor Yellow
    Write-Host "Rode scripts\pos-instalacao.cmd (nao desativa o SAC)." -ForegroundColor Cyan
    Read-Host "Enter para sair"
    exit 1
}
