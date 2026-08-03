#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Tools = Join-Path $Root "src-tauri\resources\tools"
$Ytdlp = Join-Path $Tools "yt-dlp.exe"
$Mpv = Join-Path $Tools "mpv.exe"
$MpvWd = Split-Path $Mpv
$fail = 0

function Assert($cond, [string]$msg) {
    if (-not $cond) {
        Write-Host "FAIL: $msg" -ForegroundColor Red
        $script:fail++
    } else {
        Write-Host "OK: $msg" -ForegroundColor Green
    }
}

function Search-Yt([string]$query) {
    $out = & $Ytdlp --flat-playlist --print "%(id)s`t%(title)s`t%(uploader)s" "ytsearch5:$query" 2>&1
    if ($LASTEXITCODE -ne 0) { throw "yt-dlp search failed: $out" }
    foreach ($line in ($out -split "`n")) {
        $line = $line.Trim()
        if (-not $line) { continue }
        $p = $line -split "`t", 3
        if ($p.Count -ge 2 -and $p[0]) {
            return [PSCustomObject]@{
                Id = $p[0]
                Title = $p[1]
                Uploader = $(if ($p.Count -ge 3) { $p[2] } else { "" })
                Url = "https://www.youtube.com/watch?v=$($p[0])"
            }
        }
    }
    throw "Nenhum resultado para: $query"
}

function Open-MpvSession([string]$pipe, $proc) {
    $deadline = (Get-Date).AddSeconds(25)
    while ((Get-Date) -lt $deadline) {
        $proc.Refresh()
        if ($proc.HasExited) {
            Write-Host "  mpv exit $($proc.ExitCode) aguardando pipe $pipe" -ForegroundColor Yellow
            return $null
        }
        try {
            $client = New-Object System.IO.Pipes.NamedPipeClientStream ".", $pipe, [System.IO.Pipes.PipeDirection]::InOut
            $client.Connect(2000)
            $writer = New-Object System.IO.StreamWriter($client)
            $writer.AutoFlush = $true
            $reader = New-Object System.IO.StreamReader($client)
            return [PSCustomObject]@{ Client = $client; Writer = $writer; Reader = $reader }
        } catch {
            Start-Sleep -Milliseconds 400
        }
    }
    Write-Host "  timeout IPC pipe $pipe" -ForegroundColor Yellow
    return $null
}

function Invoke-Mpv($session, [string]$json) {
    $session.Writer.WriteLine($json)
    return $session.Reader.ReadLine()
}

function Close-MpvSession($session) {
    if (-not $session) { return }
    $session.Writer.Close()
    $session.Reader.Close()
    $session.Client.Close()
    $session.Client.Dispose()
}

function Test-Playback([string]$label, [string]$url, [bool]$videoMode) {
    Get-Process mpv -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500

    $pipe = "promptub-int-$PID-$([guid]::NewGuid().ToString('N').Substring(0, 6))"
    $mpvArgs = @(
        "--idle=yes", "--keep-open=yes", "--input-ipc-server=\\.\pipe\$pipe",
        "--ytdl=yes", "--no-terminal", "--really-quiet",
        "--ytdl-raw-options=quiet=,no-warnings=,no-progress=,extractor-args=youtube:player_client=android"
    )
    if ($videoMode) {
        $mpvArgs += @("--force-window=yes", "--geometry=640x360+120+120", "--border=no", "--ontop", "--title=promptub-stream")
    } else {
        $mpvArgs += @("--force-window=no", "--no-video")
    }

    $proc = Start-Process -FilePath $Mpv -WorkingDirectory $MpvWd -ArgumentList $mpvArgs -PassThru
    $session = $null
    try {
        $session = Open-MpvSession $pipe $proc
        Assert ($null -ne $session) "$label - mpv IPC pronto"
        if (-not $session) { return }

        $format = if ($videoMode) {
            "bestvideo[height<=720]+bestaudio/b[height<=720]/best"
        } else {
            "bestaudio[ext=m4a]/bestaudio/best"
        }
        Invoke-Mpv $session "{ `"command`": [`"set_property`", `"ytdl-format`", `"$format`" ] }" | Out-Null
        Invoke-Mpv $session "{ `"command`": [`"loadfile`", `"$url`", `"replace`" ] }" | Out-Null

        $playing = $false
        for ($i = 0; $i -lt 45; $i++) {
            Start-Sleep -Milliseconds 700
            $proc.Refresh()
            if ($proc.HasExited) {
                Assert $false "$label - mpv encerrou (exit $($proc.ExitCode))"
                return
            }
            $resp = Invoke-Mpv $session '{ "command": ["get_property", "playback-time"] }'
            if ($resp -match '"data"\s*:\s*([1-9][0-9.]*)') {
                $playing = $true
                break
            }
            if ($resp -match '"data"\s*:\s*0\.[2-9]') {
                $playing = $true
                break
            }
        }
        Assert $playing "$label - stream tocando"
    } finally {
        if ($session) {
            try { Invoke-Mpv $session '{ "command": ["quit"] }' | Out-Null } catch { }
            Close-MpvSession $session
        }
        Start-Sleep -Milliseconds 400
        if (-not $proc.HasExited) {
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
        }
    }
}

Write-Host "=== promptub playback integration test ===" -ForegroundColor Cyan

Assert (Test-Path $Ytdlp) "yt-dlp encontrado"
Assert (Test-Path $Mpv) "mpv encontrado"
if ($fail -gt 0) { exit 1 }

Write-Host "Buscando videos..." -ForegroundColor Cyan
$caue = Search-Yt "cauê moura"
$zeze = Search-Yt "zeze di camargo"
Write-Host "Video: $($caue.Title) [$($caue.Id)]"
Write-Host "Musica: $($zeze.Title) [$($zeze.Id)]"

Test-Playback "VIDEO caue moura" $caue.Url $true
Test-Playback "AUDIO zeze di camargo" $zeze.Url $false

if ($fail -gt 0) {
    Write-Host ""
    Write-Host "$fail teste(s) falharam." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Playback real OK." -ForegroundColor Green
exit 0
