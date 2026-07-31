#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Exe = Join-Path $Root "bin\promptub.exe"

if (-not (Test-Path $Exe)) {
    Write-Host "Compilando..." -ForegroundColor Yellow
    & (Join-Path $PSScriptRoot "build-tauri.ps1")
}

if (-not (Test-Path $Exe)) {
    Write-Error "promptub.exe nao encontrado. Rode scripts\build-tauri.ps1"
}

# Abre o app sem deixar terminal visivel
Start-Process -FilePath $Exe -WorkingDirectory (Split-Path $Exe -Parent)
