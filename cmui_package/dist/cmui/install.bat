@echo off
rem cmui installer. Usage: install.bat "C:\path\to\game folder" [extra options, e.g. --crests]
setlocal
if "%~1"=="" (
  echo Usage: install.bat "C:\path\to\your CM0102 folder" [--check] [--crests] [--off a,b] [--uninstall]
  echo Drag your game folder onto this file, or see README.txt.
  pause
  exit /b 1
)
set PY=python
where python >nul 2>nul
if errorlevel 1 (
  where py >nul 2>nul
  if errorlevel 1 (
    echo Python 3 was not found. Install it from https://www.python.org/downloads/
    echo ^(tick "Add python.exe to PATH"^), then run:  pip install -r requirements.txt
    pause
    exit /b 1
  )
  set PY=py -3
)
%PY% -c "import pefile, PIL" >nul 2>nul
if errorlevel 1 (
  echo Python is here, but pefile and Pillow are missing. Run:  pip install -r requirements.txt
  pause
  exit /b 1
)
cd /d "%~dp0"
%PY% install.py %*
pause
