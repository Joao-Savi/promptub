#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$OutDir = Join-Path $Root "docs\screenshots"
$Exe = Join-Path $Root "bin\promptub.exe"
$Setup = Join-Path $Root "bin\promptub_0.3.0_x64-setup.exe"

$DrawingDll = Join-Path $env:WINDIR "Microsoft.NET\Framework64\v4.0.30319\System.Drawing.dll"
Add-Type -ReferencedAssemblies $DrawingDll -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
public static class WinCap {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string w);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, UIntPtr e);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L,T,R,B; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X,Y; }
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
    public const int SW_RESTORE = 9;
    public static void Prepare(IntPtr h) {
        ShowWindow(h, SW_RESTORE);
        SetForegroundWindow(h);
    }
    public static void Save(IntPtr h, string path) {
        for (int i = 0; i < 30; i++) {
            RECT r; GetWindowRect(h, out r);
            int w = r.R - r.L, ht = r.B - r.T;
            if (w >= 400 && ht >= 300) {
                using (var bmp = new Bitmap(w, ht)) {
                    using (var g = Graphics.FromImage(bmp)) {
                        IntPtr dc = g.GetHdc();
                        PrintWindow(h, dc, 2);
                        g.ReleaseHdc(dc);
                    }
                    bmp.Save(path, ImageFormat.Png);
                }
                return;
            }
            System.Threading.Thread.Sleep(200);
        }
        throw new Exception("janela muito pequena");
    }
    public static void ClickClient(IntPtr h, int cx, int cy) {
        POINT p = new POINT { X = cx, Y = cy };
        ClientToScreen(h, ref p);
        SetCursorPos(p.X, p.Y);
        mouse_event(0x0002, 0, 0, 0, UIntPtr.Zero);
        mouse_event(0x0004, 0, 0, 0, UIntPtr.Zero);
    }
}
"@

function Wait-AppWindow($Process, [int]$Seconds = 45) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        if ($Process.HasExited) { throw "promptub encerrou ao iniciar." }
        $Process.Refresh()
        if ($Process.MainWindowHandle -ne [IntPtr]::Zero) {
            return $Process.MainWindowHandle
        }
        Start-Sleep -Milliseconds 400
    }
    throw "Janela do promptub nao apareceu a tempo."
}

if (-not (Test-Path $Exe)) {
    throw "Execute scripts\build-tauri.cmd antes de capturar screenshots."
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Get-Process promptub -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

$app = Start-Process -FilePath $Exe -PassThru
try {
    $hwnd = Wait-AppWindow $app
    [WinCap]::Prepare($hwnd)
    Start-Sleep -Seconds 3
    [WinCap]::Save($hwnd, (Join-Path $OutDir "music-mode.png"))
    Write-Host "OK music-mode.png" -ForegroundColor Green
}
finally {
    if ($app -and -not $app.HasExited) {
        Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
    }
}

Get-Process promptub -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

$appVideo = Start-Process -FilePath $Exe -ArgumentList "--screenshot-video" -PassThru
try {
    $hwnd = Wait-AppWindow $appVideo
    [WinCap]::Prepare($hwnd)
    Start-Sleep -Seconds 3
    [WinCap]::Save($hwnd, (Join-Path $OutDir "video-mode.png"))
    Write-Host "OK video-mode.png" -ForegroundColor Green
}
finally {
    if ($appVideo -and -not $appVideo.HasExited) {
        Stop-Process -Id $appVideo.Id -Force -ErrorAction SilentlyContinue
    }
}

if (Test-Path $Setup) {
    Get-Process -Name "*promptub*setup*" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    $setupProc = Start-Process -FilePath $Setup -PassThru
    try {
        $setupHwnd = Wait-AppWindow $setupProc 60
        [WinCap]::Prepare($setupHwnd)
        Start-Sleep -Seconds 2
        [WinCap]::Save($setupHwnd, (Join-Path $OutDir "installer.png"))
        Write-Host "OK installer.png" -ForegroundColor Green
    }
    finally {
        if ($setupProc -and -not $setupProc.HasExited) {
            Stop-Process -Id $setupProc.Id -Force -ErrorAction SilentlyContinue
        }
    }
} else {
    Write-Host "Instalador nao encontrado; mantendo installer.png anterior." -ForegroundColor Yellow
}

Write-Host "Screenshots em $OutDir" -ForegroundColor Cyan
