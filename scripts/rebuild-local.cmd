@echo off
setlocal
cd /d "%~dp0\.."
set CARGO_TARGET_DIR=%CD%\src-tauri\target
call "%~dp0build-tauri.cmd"
exit /b %ERRORLEVEL%
