@echo off
cd /d "%~dp0\.."
if not exist "bin\promptub.exe" (
  echo Compilando...
  call "%~dp0build-tauri.cmd"
)
start "" "%~dp0..\bin\promptub.exe"
