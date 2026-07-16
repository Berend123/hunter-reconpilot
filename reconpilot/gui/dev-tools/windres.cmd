@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "OUT_FILE="

:parse
if "%~1"=="" goto done
if /I "%~1"=="--output" (
  set "OUT_FILE=%~2"
  shift
  shift
  goto parse
)
shift
goto parse

:done
if "%OUT_FILE%"=="" (
  echo windres shim: missing --output path 1>&2
  exit /b 1
)

for %%I in ("%OUT_FILE%") do (
  if not exist "%%~dpI" mkdir "%%~dpI" >nul 2>nul
)

> "%OUT_FILE%" (
  <nul set /p "=!<arch>"
  echo/
)

exit /b 0
