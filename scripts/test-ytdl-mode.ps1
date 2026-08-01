#Requires -Version 5.1
param([string]$Url = "https://www.youtube.com/watch?v=tHmaLPgJqQU")
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Mpv = Join-Path $Root "src-tauri\resources\tools\mpv.exe"
$MpvWd = Split-Path $Mpv
Get-Process mpv -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 400

$pipe = "promptub-ytdl-$PID"
$raw = "quiet=,no-warnings=,no-progress=,extractor-args=youtube:player_client=android"
$proc = Start-Process -FilePath $Mpv -WorkingDirectory $MpvWd -ArgumentList @(
    "--idle=yes", "--keep-open=yes", "--input-ipc-server=\\.\pipe\$pipe",
    "--ytdl=yes", "--no-terminal", "--really-quiet", "--force-window=no",
    "--ytdl-raw-options=$raw"
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
            $session = [PSCustomObject]@{ Writer = $writer; Reader = $reader; Client = $client }
            break
        } catch { Start-Sleep -Milliseconds 400 }
    }
    if (-not $session) { throw "IPC timeout" }

    function Mpv([string]$json) { $session.Writer.WriteLine($json); return $session.Reader.ReadLine() }

    Mpv '{ "command": ["set_property", "ytdl-format", "bestaudio[ext=m4a]/bestaudio/best" ] }' | Out-Null
    Mpv '{ "command": ["set_property", "video", false ] }' | Out-Null
    $load = Mpv "{ `"command`": [`"loadfile`", `"$Url`", `"replace`" ] }"
    Write-Host "loadfile: $load"

    for ($i = 0; $i -lt 45; $i++) {
        Start-Sleep -Milliseconds 700
        $proc.Refresh()
        if ($proc.HasExited) { throw "mpv encerrou durante ytdl load" }
        $resp = Mpv '{ "command": ["get_property", "playback-time"] }'
        Write-Host "[$i] $resp"
        if ($resp -match '"data"\s*:\s*([1-9][0-9.]*)' -or $resp -match '"data"\s*:\s*0\.[2-9]') {
            Write-Host "YTDLP MODE OK" -ForegroundColor Green
            exit 0
        }
        $idle = Mpv '{ "command": ["get_property", "idle-active"] }'
        if ($idle -match '"data"\s*:\s*true' -and $i -gt 10) { throw "mpv voltou ao idle (ytdl falhou)" }
    }
    throw "timeout aguardando playback"
}
finally {
    if ($session) {
        try { $session.Writer.WriteLine('{ "command": ["quit"] }') } catch {}
        $session.Writer.Close(); $session.Reader.Close(); $session.Client.Close()
    }
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
}
