@echo off
call "D:\VSBuildTools\VC\Auxiliary\Build\vcvars64.bat" 2>nul
if errorlevel 1 (
  call "%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" 2>nul
)
set CARGO_TARGET_DIR=D:\aquerty-cargo-target
cd /d "%~dp0src-tauri"
echo Generating 50 lifetime keys...
cargo run --bin gen-license -- batch-lifetime 50 GUM
echo.
pause
