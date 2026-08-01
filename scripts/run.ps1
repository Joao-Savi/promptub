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

try {
    Start-Process -FilePath $Exe -WorkingDirectory (Split-Path $Exe -Parent) -ErrorAction Stop
} catch {
    if ($_.Exception.Message -match "Controle de Aplicativo|4551|App Control") {
        Write-Host "Smart App Control bloqueou o promptub.exe." -ForegroundColor Yellow
        Write-Host "Rode scripts\pos-instalacao.cmd (nao desativa o SAC)." -ForegroundColor Cyan
        & (Join-Path $PSScriptRoot "open-promptub.cmd")
    } else {
        throw
    }
}
