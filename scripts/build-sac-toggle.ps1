#Requires -RunAsAdministrator
#Requires -Version 5.1
# DEV ONLY — desativa SAC para compilar Rust/Cargo. Nao use pos-instalacao do usuario final.
$ErrorActionPreference = "Stop"

function Set-SacState {
    param([int]$State)
    $key = "HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy"
    Set-ItemProperty -Path $key -Name "VerifiedAndReputablePolicyState" -Value $State -Type DWord
    & "$env:SystemRoot\System32\CiTool.exe" -r
    if ($LASTEXITCODE -ne 0) { throw "CiTool.exe -r falhou (exit $LASTEXITCODE)" }
}

$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$policyKey = "HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy"
$previous = (Get-ItemProperty -Path $policyKey -Name "VerifiedAndReputablePolicyState" -ErrorAction SilentlyContinue).VerifiedAndReputablePolicyState
Write-Host "SAC atual: $previous" -ForegroundColor DarkGray

Write-Host "Desativando Smart App Control..." -ForegroundColor Yellow
Set-SacState -State 0
Write-Host "SAC desativado." -ForegroundColor Green

$env:Path = "E:\Tools\NodeJS;E:\Tools\Rust\.cargo\bin;" + $env:Path
$env:RUSTUP_HOME = "E:\Tools\Rust\.rustup"
$env:CARGO_HOME = "E:\Tools\Rust\.cargo"
$env:CARGO_TARGET_DIR = Join-Path $Root "src-tauri\target"

Push-Location $Root
try {
    . (Join-Path $PSScriptRoot "setup-vs.ps1")
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    Write-Host "Compilando promptub..." -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot "build-tauri.ps1")
    $buildExit = $LASTEXITCODE
}
finally {
    Pop-Location
}

Write-Host ""
Write-Host "SAC permanece DESLIGADO para voce conseguir abrir o promptub." -ForegroundColor Yellow
Write-Host "Use scripts\open-promptub.cmd ou o atalho na area de trabalho." -ForegroundColor Cyan

if ($buildExit -ne 0) { exit $buildExit }
Write-Host "Build concluido com sucesso." -ForegroundColor Green
