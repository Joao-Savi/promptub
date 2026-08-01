#Requires -Version 5.1
param(
    [string]$VideoId = "tHmaLPgJqQU"
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Tools = Join-Path $Root "src-tauri\resources\tools"
$Ytdlp = Join-Path $Tools "yt-dlp.exe"
$Mpv = Join-Path $Tools "mpv.exe"
$MpvWd = Split-Path $Mpv
$url = "https://www.youtube.com/watch?v=$VideoId"

Get-Process mpv -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

Write-Host "Resolvendo stream..." -ForegroundColor Cyan
$stream = & $Ytdlp --quiet --no-warnings --no-playlist -f "bestaudio[ext=m4a]/bestaudio/best" -g --extractor-args "youtube:player_client=android" $url 2>&1
if ($LASTEXITCODE -ne 0 -or -not ($stream -match "^https?://")) {
    Write-Host "FAIL stream: $stream" -ForegroundColor Red
    exit 1
}
Write-Host "Stream OK" -ForegroundColor Green

$pipe = "promptub-one-$PID"
$proc = Start-Process -FilePath $Mpv -WorkingDirectory $MpvWd -ArgumentList @(
    "--idle=yes", "--keep-open=yes", "--input-ipc-server=\\.\pipe\$pipe",
    "--no-terminal", "--really-quiet", "--force-window=no", "--no-video"
) -PassThru -WindowStyle Hidden

$session = $null
try {
    $deadline = (Get-Date).AddSeconds(25)
    while ((Get-Date) -lt $deadline) {
        $proc.Refresh()
        if ($proc.HasExited) { throw "mpv encerrou (exit $($proc.ExitCode))" }
        try {
            $client = [System.IO.Pipes.NamedPipeClientStream]::new(".", $pipe, [System.IO.Pipes.PipeDirection]::InOut)
            $client.Connect(2000)
            $writer = New-Object System.IO.StreamWriter($client)
            $writer.AutoFlush = $true
            $reader = New-Object System.IO.StreamReader($client)
            $session = [PSCustomObject]@{ Client = $client; Writer = $writer; Reader = $reader }
            break
        } catch {
            Start-Sleep -Milliseconds 400
        }
    }
    if (-not $session) { throw "IPC timeout" }

    function Invoke-Mpv([string]$json) {
        $session.Writer.WriteLine($json)
        return $session.Reader.ReadLine()
    }

    $loadResp = Invoke-Mpv "{ `"command`": [`"loadfile`", `"$stream`", `"replace`" ] }"
    Write-Host "loadfile: $loadResp"

    $playing = $false
    for ($i = 0; $i -lt 40; $i++) {
        Start-Sleep -Milliseconds 700
        $proc.Refresh()
        if ($proc.HasExited) { throw "mpv encerrou durante playback" }
        $resp = Invoke-Mpv '{ "command": ["get_property", "playback-time"] }'
        if ($resp -match '"data"\s*:\s*([1-9][0-9.]*)' -or $resp -match '"data"\s*:\s*0\.[2-9]') {
            $playing = $true
            Write-Host "TOCANDO: $resp" -ForegroundColor Green
            break
        }
    }
    if (-not $playing) { throw "nao comecou a tocar" }
    Write-Host "OK: $VideoId" -ForegroundColor Green
    exit 0
}
finally {
    if ($session) {
        try { $session.Writer.WriteLine('{ "command": ["quit"] }') } catch {}
        $session.Writer.Close(); $session.Reader.Close(); $session.Client.Close(); $session.Client.Dispose()
    }
    Start-Sleep -Milliseconds 300
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
}
