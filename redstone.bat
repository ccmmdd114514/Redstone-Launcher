@echo off
setlocal

set "RS_EXE=%~dp0redstone.exe"
if not exist "%RS_EXE%" set "RS_EXE=%~dp0dist\redstone.exe"

if not exist "%RS_EXE%" (
  echo [Redstone] redstone.exe not found.
  echo [Redstone] Please run: cargo build --release
  echo [Redstone] then copy target\release\redstone.exe here.
  echo.
  pause
  exit /b 1
)

"%RS_EXE%" %*
exit /b %ERRORLEVEL%
