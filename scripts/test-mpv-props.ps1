#Requires -Version 5.1
$Mpv = "E:\Projects\promptub\src-tauri\resources\tools\mpv.exe"
$Wd = Split-Path $Mpv
Get-Process mpv -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
$pipe = "mpv-probe-$PID"
$args = @(
    "--idle=yes", "--input-ipc-server=\\.\pipe\$pipe",
    "--no-terminal", "--really-quiet", "--ytdl=yes",
    "--force-window=yes", "--geometry=640x360+100+100", "--border=no", "--ontop"
)
$p = Start-Process $Mpv -WorkingDirectory $Wd -ArgumentList $args -PassThru
Start-Sleep 2
$c = [System.IO.Pipes.NamedPipeClientStream]::new(".", $pipe, [System.IO.Pipes.PipeDirection]::InOut)
$c.Connect(5000)
$w = New-Object IO.StreamWriter($c); $w.AutoFlush = $true
$r = New-Object IO.StreamReader($c)
function M([string]$j) { $w.WriteLine($j); return $r.ReadLine() }
$props = @(
    '{ "command": ["set_property", "force-window", true ] }',
    '{ "command": ["set_property", "video", true ] }',
    '{ "command": ["set_property", "geometry", "800x450+200+150" ] }',
    '{ "command": ["set_property", "ytdl-format", "b[height<=720]/best[height<=720]/bestvideo[height<=720]+bestaudio/best" ] }',
    '{ "command": ["set_property", "focus-on-open", false ] }'
)
foreach ($cmd in $props) {
    Write-Host "$cmd"
    Write-Host " -> $(M $cmd)"
}
M '{ "command": ["loadfile", "https://www.youtube.com/watch?v=zs6nw_ATMjc", "replace" ] }' | Out-Null
Start-Sleep 8
Write-Host "time: $(M '{ "command": ["get_property", "playback-time"] }')"
$w.WriteLine('{ "command": ["quit"] }')
if (-not $p.HasExited) { Stop-Process $p.Id -Force }
