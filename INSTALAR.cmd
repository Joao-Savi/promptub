@echo off
title Instalar promptub
cd /d "%~dp0"

if exist "bin\promptub_0.3.0_x64-setup.exe" (
  echo Abrindo instalador do promptub...
  start "" "bin\promptub_0.3.0_x64-setup.exe"
  exit /b 0
)

for %%F in (bin\*setup.exe) do (
  echo Abrindo instalador: %%~nxF
  start "" "%%F"
  exit /b 0
)

echo.
echo Instalador nao encontrado. Compilando...
call scripts\build-tauri.cmd
if errorlevel 1 exit /b 1
start "" "bin\promptub_0.3.0_x64-setup.exe"
