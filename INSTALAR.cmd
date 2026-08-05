@echo off
title Instalar promptub
cd /d "%~dp0"

set "WANT=0.5.8"
set "SETUP="

if exist "bin\promptub_%WANT%_x64-setup.exe" (
  set "SETUP=bin\promptub_%WANT%_x64-setup.exe"
  goto :found
)

if exist "src-tauri\target\release\bundle\nsis\promptub_%WANT%_x64-setup.exe" (
  set "SETUP=src-tauri\target\release\bundle\nsis\promptub_%WANT%_x64-setup.exe"
  goto :found
)

echo.
echo Instalador v%WANT% nao encontrado.
if exist "bin\promptub_0.3.0_x64-setup.exe" (
  echo AVISO: so existe setup antigo 0.3.0 em bin\ — sera gerado v%WANT%.
)
echo Compilando promptub v%WANT%...
call scripts\build-tauri.cmd
if errorlevel 1 exit /b 1

if exist "bin\promptub_%WANT%_x64-setup.exe" (
  set "SETUP=bin\promptub_%WANT%_x64-setup.exe"
  goto :found
)
for /f "delims=" %%F in ('dir /b /o-d "bin\*setup.exe" 2^>nul') do (
  set "SETUP=bin\%%F"
  goto :found
)
echo ERRO: setup.exe v%WANT% nao gerado.
exit /b 1

:found
echo.
echo Abrindo instalador:
echo   %SETUP%
echo.
echo IMPORTANTE: reinstala por cima — remove legado e limpa cache do WebView2.
echo Na barra inferior deve aparecer: v%WANT% · vermelho · noite
echo.
echo Apos instalar, se o Windows bloquear parte do app:
echo   scripts\pos-instalacao.cmd
echo.
start "" "%SETUP%"
