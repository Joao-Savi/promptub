@echo off
cd /d "%~dp0\.."
if not exist "bin\promptub.exe" goto build
if not exist "bin\promptub.build.stamp" goto build
start "" "%~dp0..\bin\promptub.exe"
exit /b 0

:build
echo Build de producao necessaria...
call "%~dp0build-tauri.cmd"
if errorlevel 1 exit /b 1
start "" "%~dp0..\bin\promptub.exe"
