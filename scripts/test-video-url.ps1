#Requires -Version 5.1
param([string]$Url = "https://www.youtube.com/watch?v=zs6nw_ATMjc")
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Mpv = Join-Path $Root "src-tauri\resources\tools\mpv.exe"
$MpvWd = Split-Path $Mpv
Get-Process mpv -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 400

$pipe = "promptub-vid-$PID"
$raw = "quiet=,no-warnings=,no-progress=,extractor-args=youtube:player_client=android"
$proc = Start-Process -FilePath $Mpv -WorkingDirectory $MpvWd -ArgumentList @(
    "--idle=yes", "--keep-open=yes", "--input-ipc-server=\\.\pipe\$pipe",
    "--ytdl=yes", "--no-terminal", "--really-quiet",
    "--ytdl-raw-options=$raw",
    "--force-window=yes", "--geometry=640x360+120+120", "--border=no", "--ontop"
) -PassThru

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

    $format = "bestvideo[height<=720]+bestaudio/b[height<=720]/best"
    Mpv "{ `"command`": [`"set_property`", `"ytdl-format`", `"$format`" ] }" | Out-Null
    Mpv "{ `"command`": [`"set_property`", `"video`", true ] }" | Out-Null
    $load = Mpv "{ `"command`": [`"loadfile`", `"$Url`", `"replace`" ] }"
    Write-Host "loadfile: $load"

    for ($i = 0; $i -lt 45; $i++) {
        Start-Sleep -Milliseconds 700
        $proc.Refresh()
        if ($proc.HasExited) { throw "mpv encerrou durante playback" }
        $resp = Mpv '{ "command": ["get_property", "playback-time"] }'
        if ($resp -match '"data"\s*:\s*([1-9][0-9.]*)' -or $resp -match '"data"\s*:\s*0\.[2-9]') {
            Write-Host "VIDEO OK: $resp" -ForegroundColor Green
            exit 0
        }
        if ($i % 5 -eq 0) { Write-Host "[$i] $resp" }
    }
    throw "timeout"
}
finally {
    if ($session) {
        try { $session.Writer.WriteLine('{ "command": ["quit"] }') } catch {}
        $session.Writer.Close(); $session.Reader.Close(); $session.Client.Close()
    }
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
}
