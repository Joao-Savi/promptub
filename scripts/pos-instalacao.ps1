#Requires -Version 5.1
$ErrorActionPreference = "Stop"

function Unblock-Tree {
    param([string]$Dir)
    if (-not (Test-Path $Dir)) { return 0 }
    Write-Host "Desbloqueando: $Dir" -ForegroundColor Cyan
    $count = 0
    Get-ChildItem $Dir -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
        Unblock-File -LiteralPath $_.FullName -ErrorAction SilentlyContinue
        $count++
    }
    return $count
}

function Test-ToolOnPath {
    param([string]$Name)
    $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Install-TrustedTool {
    param(
        [string]$WingetId,
        [string]$ExeName
    )
    if (Test-ToolOnPath $ExeName) {
        Write-Host "$ExeName ja esta no PATH." -ForegroundColor Green
        return
    }
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        Write-Host "winget nao encontrado - instale $ExeName manualmente se o app nao tocar midia." -ForegroundColor Yellow
        return
    }
    Write-Host "Instalando $ExeName via winget (reputacao Microsoft, SAC-friendly)..." -ForegroundColor Cyan
    $wingetArgs = @(
        "install", "--id", $WingetId, "-e",
        "--accept-source-agreements", "--accept-package-agreements",
        "--scope", "user"
    )
    & winget @wingetArgs
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne -1978335189) {
        Write-Host "winget retornou $LASTEXITCODE para $WingetId (pode ja estar instalado)." -ForegroundColor DarkGray
    }
}

$Root = Split-Path -Parent $PSScriptRoot
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\promptub"
$BinDir = Join-Path $Root "bin"

Write-Host ""
Write-Host "promptub - pos-instalacao (mantem Smart App Control ATIVO)" -ForegroundColor White
Write-Host ""

$unblocked = (Unblock-Tree $InstallDir) + (Unblock-Tree $BinDir)
Write-Host "Arquivos processados: $unblocked" -ForegroundColor DarkGray

$policyKey = "HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy"
$sac = (Get-ItemProperty -Path $policyKey -Name "VerifiedAndReputablePolicyState" -ErrorAction SilentlyContinue).VerifiedAndReputablePolicyState
$sacLabel = switch ($sac) {
    0 { "desligado" }
    1 { "ativo (enforcement)" }
    2 { "avaliacao" }
    default { "desconhecido ($sac)" }
}
Write-Host "Smart App Control: $sacLabel" -ForegroundColor DarkGray

Write-Host ""
Write-Host "Dependencias no PATH (prioridade sobre tools/ embutidos):" -ForegroundColor White
Install-TrustedTool -WingetId "yt-dlp.yt-dlp" -ExeName "yt-dlp"

$ToolsDir = Join-Path $InstallDir "tools"
if (Test-Path $ToolsDir) {
    Get-ChildItem $ToolsDir -File -ErrorAction SilentlyContinue | Where-Object {
        $_.Name -ne "yt-dlp.exe"
    } | Remove-Item -Force -ErrorAction SilentlyContinue
    Write-Host "Pasta tools/ limpa (somente yt-dlp)." -ForegroundColor DarkGray
}

$WebViewData = Join-Path $env:LOCALAPPDATA "com.promptub"
if (Test-Path $WebViewData) {
    Write-Host "Limpando cache WebView2..." -ForegroundColor Cyan
    Remove-Item -LiteralPath $WebViewData -Recurse -Force -ErrorAction SilentlyContinue
}

$Exe = @(
    Join-Path $InstallDir "promptub.exe"
    Join-Path $BinDir "promptub.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $Exe) {
    Write-Host ""
    Write-Host "promptub.exe nao encontrado. Instale primeiro pelo setup." -ForegroundColor Red
    Read-Host "Enter para sair"
    exit 1
}

Write-Host ""
Write-Host "Iniciando promptub..." -ForegroundColor Green
try {
    Start-Process -FilePath $Exe -WorkingDirectory (Split-Path $Exe -Parent) -ErrorAction Stop
    Write-Host "OK." -ForegroundColor Green
} catch {
    Write-Host ""
    Write-Host "Nao foi possivel abrir o promptub.exe." -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Com SAC ativo, builds locais sem assinatura podem ser bloqueados." -ForegroundColor Yellow
    Write-Host "O app usa yt-dlp do winget ou embutido (nao desativa a seguranca)." -ForegroundColor DarkGray
    Write-Host "Se antes funcionava: reinstale por cima SEM apagar a pasta, ou use release assinada." -ForegroundColor DarkGray
    Read-Host "Enter para sair"
    exit 1
}
