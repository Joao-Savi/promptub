#Requires -Version 5.1
# Configura PATH para compilar Rust/Tauri no Windows (link.exe, rc.exe, etc.)
$ErrorActionPreference = "Stop"

function Find-VcVars {
    $candidates = @(
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat",
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat",
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    )
    foreach ($p in $candidates) {
        if (Test-Path $p) { return $p }
    }
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $install = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
        if ($install) {
            $vc = Join-Path $install "VC\Auxiliary\Build\vcvars64.bat"
            if (Test-Path $vc) { return $vc }
        }
    }
    return $null
}

$VcVars = Find-VcVars
if (-not $VcVars) {
    Write-Host ""
    Write-Host "Visual C++ Build Tools nao encontrado." -ForegroundColor Red
    Write-Host "Instale com (PowerShell como admin):" -ForegroundColor Yellow
    Write-Host '  winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"' -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Depois feche e reabra o terminal e rode de novo." -ForegroundColor Yellow
    exit 1
}

Write-Host "Usando: $VcVars" -ForegroundColor DarkGray

# Exporta PATH/LIB/INCLUDE do vcvars para este processo
$envLines = cmd /c "`"$VcVars`" >nul 2>&1 && set" | Where-Object { $_ -match '^(PATH|LIB|INCLUDE|LIBPATH)=' }
foreach ($line in $envLines) {
    $name, $value = $line -split '=', 2
    Set-Item -Path "Env:$name" -Value $value
}

if (-not (Get-Command link.exe -ErrorAction SilentlyContinue)) {
    Write-Host "link.exe ainda nao encontrado apos vcvars." -ForegroundColor Red
    exit 1
}

Write-Host "link.exe OK" -ForegroundColor Green
