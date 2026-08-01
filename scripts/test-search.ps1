#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Ytdlp = "E:\Projects\promptub\src-tauri\resources\tools\yt-dlp.exe"
$fields = "%(id)s`t%(title)s`t%(uploader)s`t%(duration_string)s`t%(live_status)s"
$query = if ($args[0]) { $args[0] } else { "zeze di camargo" }

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $Ytdlp
$psi.Arguments = "--quiet --no-warnings --no-progress --encoding utf-8 --flat-playlist --print `"$fields`" `"ytsearch10:$query`""
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.UseShellExecute = $false
$psi.CreateNoWindow = $true
$p = [Diagnostics.Process]::Start($psi)
$stdout = $p.StandardOutput.ReadToEnd()
$stderr = $p.StandardError.ReadToEnd()
$p.WaitForExit()

Write-Host "exit: $($p.ExitCode)"
Write-Host "stderr: [$stderr]"
$lines = @($stdout -split "`n" | Where-Object { $_.Trim() })
Write-Host "stdout lines: $($lines.Count)"
$lines | Select-Object -First 5
foreach ($line in $lines) {
    $parts = $line -split "`t"
    if ($parts.Count -lt 2) { Write-Host "PARSE FAIL: $line" -ForegroundColor Red }
}
