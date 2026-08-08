@echo off
REM Dev local Aquerty Stop
call "D:\VSBuildTools\VC\Auxiliary\Build\vcvars64.bat" 2>nul
if errorlevel 1 (
  call "%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" 2>nul
)
if errorlevel 1 (
  echo ERROR: vcvars64.bat introuvable.
  pause
  exit /b 1
)
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
set CARGO_TARGET_DIR=D:\aquerty-cargo-target
cd /d "%~dp0"
if not exist node_modules (
  echo npm install...
  call npm install
)
echo Lancement tauri dev (1er compile = long)...
npm run tauri dev
pause
