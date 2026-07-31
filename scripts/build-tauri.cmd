@echo off
cd /d "%~dp0\.."
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-tauri.ps1"
exit /b %ERRORLEVEL%
