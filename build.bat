@echo off
REM Build Aquerty Stop (requires VS Build Tools with C++ workload)
call "D:\VSBuildTools\VC\Auxiliary\Build\vcvars64.bat" 2>nul
if errorlevel 1 (
  call "%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" 2>nul
)
if errorlevel 1 (
  echo ERROR: Visual Studio Build Tools / vcvars64.bat introuvable.
  echo Installe "Desktop development with C++" puis relance.
  exit /b 1
)
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
cd /d "%~dp0"
npm run tauri build
echo.
echo Installateurs :
dir /b "D:\aquerty-cargo-target\release\bundle\nsis\*.exe" 2>nul
dir /b "D:\aquerty-cargo-target\release\bundle\msi\*.msi" 2>nul
pause
