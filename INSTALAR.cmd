@echo off
title Instalar promptub
cd /d "%~dp0"

set "SETUP="
for /f "delims=" %%F in ('dir /b /o-d "bin\*setup.exe" 2^>nul') do (
  set "SETUP=bin\%%F"
  goto :found
)

if exist "src-tauri\target\release\bundle\nsis\promptub_0.3.1_x64-setup.exe" (
  set "SETUP=src-tauri\target\release\bundle\nsis\promptub_0.3.1_x64-setup.exe"
  goto :found
)

echo.
echo Instalador nao encontrado. Compilando v0.3.1...
call scripts\build-tauri.cmd
if errorlevel 1 exit /b 1
for /f "delims=" %%F in ('dir /b /o-d "bin\*setup.exe" 2^>nul') do (
  set "SETUP=bin\%%F"
  goto :found
)
echo ERRO: setup.exe nao gerado.
exit /b 1

:found
echo.
echo Abrindo instalador mais recente:
echo   %SETUP%
echo.
echo IMPORTANTE: instale por cima da versao antiga.
echo Na barra inferior deve aparecer: v0.3.1 · web
echo.
echo Apos instalar, se o Windows bloquear parte do app:
echo   scripts\pos-instalacao.cmd
echo ^(desbloqueia + yt-dlp/mpv via winget — SAC continua ativo^)
echo.
start "" "%SETUP%"
