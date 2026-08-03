@echo off
title promptub — pos-instalacao
cd /d "%~dp0\.."
echo.
echo Desbloqueia arquivos e instala yt-dlp via winget se faltarem.
echo Mantem Smart App Control ATIVO.
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0pos-instalacao.ps1"
pause
