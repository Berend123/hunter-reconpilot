@echo off
setlocal

set "REPO_ROOT=%~dp0"
cd /d "%REPO_ROOT%"

echo ReconPilot GUI launcher
echo.
echo This starts the local Tauri GUI in dev mode.
echo Dry-run remains the default. Review scope and command previews before execution.
echo.

where powershell.exe >nul 2>nul
if errorlevel 1 (
  echo [error] Windows PowerShell was not found on PATH.
  echo Install or restore PowerShell, then try again.
  echo.
  pause
  exit /b 1
)

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%REPO_ROOT%scripts\launch-gui.ps1"
set "EXIT_CODE=%ERRORLEVEL%"

if not "%EXIT_CODE%"=="0" (
  echo.
  echo ReconPilot GUI did not launch successfully. See the messages above.
  echo.
  pause
)

exit /b %EXIT_CODE%
