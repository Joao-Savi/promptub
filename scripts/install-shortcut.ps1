#Requires -Version 5.1
$ErrorActionPreference = "Stop"

function Set-PromptubShortcutIcon {
    param(
        [Parameter(Mandatory = $true)][string]$ShortcutPath,
        [Parameter(Mandatory = $true)][string]$TargetPath,
        [Parameter(Mandatory = $true)][string]$IconPath,
        [string]$WorkingDirectory = ""
    )
    if (-not (Test-Path $IconPath)) {
        throw "Icone nao encontrado: $IconPath"
    }
    $iconFull = (Resolve-Path -LiteralPath $IconPath).Path
    $targetFull = (Resolve-Path -LiteralPath $TargetPath).Path
    $wd = if ($WorkingDirectory -and (Test-Path $WorkingDirectory)) {
        (Resolve-Path -LiteralPath $WorkingDirectory).Path
    } else {
        Split-Path $targetFull -Parent
    }

    if (Test-Path $ShortcutPath) {
        Remove-Item -LiteralPath $ShortcutPath -Force
    }

    $WshShell = New-Object -ComObject WScript.Shell
    $Shortcut = $WshShell.CreateShortcut($ShortcutPath)
    $Shortcut.TargetPath = $targetFull
    $Shortcut.WorkingDirectory = $wd
    $Shortcut.Description = "YouTube e YouTube Music - promptub"
    $Shortcut.IconLocation = "$iconFull,0"
    $Shortcut.Save()
}

$Root = Split-Path -Parent $PSScriptRoot
$IconSrc = Join-Path $Root "src-tauri\icons\icon.ico"
$BinExe = Join-Path $Root "bin\promptub.exe"
$InstallExe = Join-Path $env:LOCALAPPDATA "Programs\promptub\promptub.exe"
$InstallIcon = Join-Path $env:LOCALAPPDATA "Programs\promptub\promptub.ico"

if (-not (Test-Path $IconSrc)) {
    Write-Host "Gerando icone vermelho..." -ForegroundColor Yellow
    & (Join-Path $PSScriptRoot "generate-icon.ps1")
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$TargetExe = if (Test-Path $InstallExe) { $InstallExe } elseif (Test-Path $BinExe) { $BinExe } else {
    Write-Host "Gerando build..." -ForegroundColor Yellow
    & (Join-Path $PSScriptRoot "build-tauri.ps1")
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    if (Test-Path $InstallExe) { $InstallExe } elseif (Test-Path $BinExe) { $BinExe } else {
        throw "promptub.exe nao encontrado apos build"
    }
}

$IconForShortcut = $IconSrc
if ($TargetExe -eq $InstallExe) {
    $installDir = Split-Path $InstallExe -Parent
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Copy-Item $IconSrc (Join-Path $installDir "promptub.ico") -Force
    $IconForShortcut = Join-Path $installDir "promptub.ico"
}

$Desktop = [Environment]::GetFolderPath("Desktop")
$ShortcutPath = Join-Path $Desktop "promptub.lnk"

Set-PromptubShortcutIcon -ShortcutPath $ShortcutPath -TargetPath $TargetExe -IconPath $IconForShortcut

Write-Host "Atalho atualizado: $ShortcutPath" -ForegroundColor Green
Write-Host "Alvo: $TargetExe" -ForegroundColor DarkGray
Write-Host "Icone: $IconForShortcut" -ForegroundColor DarkGray
