#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

. (Join-Path $PSScriptRoot "setup-vs.ps1")

Push-Location $Root
try {
    npm run tauri -- dev
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
