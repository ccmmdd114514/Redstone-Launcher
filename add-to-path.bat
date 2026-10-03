@echo off
setlocal

set "RS_DIR=%~dp0"
if exist "%~dp0dist\redstone.exe" set "RS_DIR=%~dp0dist"

if not exist "%RS_DIR%\redstone.exe" (
  echo [Redstone] redstone.exe not found. Run cargo build --release first.
  pause
  exit /b 1
)

echo [Redstone] Adding this folder to your user PATH:
echo [Redstone] %RS_DIR%
echo.

powershell -NoProfile -Command "$p=[Environment]::GetEnvironmentVariable('Path','User'); if($p -notlike '*%RS_DIR%*'){ [Environment]::SetEnvironmentVariable('Path', $p + ';%RS_DIR%', 'User'); Write-Host 'Added.' } else { Write-Host 'Already present.' }"

echo.
echo [Redstone] Open a NEW terminal and type: redstone doctor
echo.
pause
