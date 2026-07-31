#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Exe = Join-Path $Root "bin\promptub.exe"
$Stamp = Join-Path $Root "bin\promptub.build.stamp"
$fail = 0

function Assert($cond, [string]$msg) {
    if (-not $cond) {
        Write-Host "FAIL: $msg" -ForegroundColor Red
        $script:fail++
    } else {
        Write-Host "OK: $msg" -ForegroundColor Green
    }
}

Write-Host "=== promptub smoke test ===" -ForegroundColor Cyan

Assert (Test-Path $Exe) "bin\promptub.exe existe"
Assert (Test-Path $Stamp) "build de producao (stamp)"
if (Test-Path $Exe) {
    Assert ((Get-Item $Exe).Length -gt 1MB) "exe tem tamanho valido"
}

$toolsDir = Join-Path $Root "src-tauri\resources\tools"
$ytdlp = Join-Path $toolsDir "yt-dlp.exe"
$mpv = Join-Path $toolsDir "mpv.exe"
Assert (Test-Path $ytdlp) "yt-dlp bundlado"
Assert (Test-Path $mpv) "mpv bundlado"

Get-Process promptub -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 400

function Test-Launch([string]$label, [string[]]$launchArgs) {
    if ($launchArgs) {
        $p = Start-Process -FilePath $Exe -ArgumentList $launchArgs -PassThru
    } else {
        $p = Start-Process -FilePath $Exe -PassThru
    }
    $deadline = (Get-Date).AddSeconds(12)
    $hwnd = [IntPtr]::Zero
    while ((Get-Date) -lt $deadline) {
        if ($p.HasExited) {
            Assert $false "$label - app encerrou (exit $($p.ExitCode))"
            return
        }
        $p.Refresh()
        if ($p.MainWindowHandle -ne [IntPtr]::Zero) {
            $hwnd = $p.MainWindowHandle
            break
        }
        Start-Sleep -Milliseconds 300
    }
    if ($hwnd -eq [IntPtr]::Zero) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        Assert $false "$label - janela nao abriu"
        return
    }
    Start-Sleep -Seconds 4
    $p.Refresh()
    Assert (-not $p.HasExited) "$label - app permanece aberto"
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 400
}

Test-Launch "modo musica" $null
Test-Launch "modo video (boot)" @("--screenshot-video")

if ($fail -gt 0) {
    Write-Host ""
    Write-Host "$fail teste(s) falharam." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Todos os testes passaram." -ForegroundColor Green
exit 0
