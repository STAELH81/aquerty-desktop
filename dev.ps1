# Dev local Aquerty Stop (PowerShell)
$ErrorActionPreference = "Stop"

$vcvars = "D:\VSBuildTools\VC\Auxiliary\Build\vcvars64.bat"
if (-not (Test-Path $vcvars)) {
  $vcvars = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
}
if (-not (Test-Path $vcvars)) {
  Write-Error "vcvars64.bat introuvable"
  exit 1
}

$root = $PSScriptRoot
if (-not $root) {
  $root = Split-Path -Parent $MyInvocation.MyCommand.Path
}
Set-Location -LiteralPath $root

# Free Vite port if a previous dev is still running
$vite = Get-NetTCPConnection -LocalPort 1420 -ErrorAction SilentlyContinue |
  Select-Object -ExpandProperty OwningProcess -Unique
foreach ($procId in $vite) {
  if ($procId) {
    Write-Host "Kill process $procId on port 1420..."
    Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
  }
}
Get-Process -Name "aquerty-stop" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

$env:CARGO_TARGET_DIR = "D:\aquerty-cargo-target"
$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path

if (-not (Test-Path "node_modules")) {
  npm install
}

Write-Host "Init MSVC + tauri dev..."
# One cmd session: vcvars then npm (no trailing spaces in env)
$cmd = @(
  "call `"$vcvars`"",
  "set `"CARGO_TARGET_DIR=D:\aquerty-cargo-target`"",
  "set `"PATH=%USERPROFILE%\.cargo\bin;%PATH%`"",
  "cd /d `"$root`"",
  "npm run tauri dev"
) -join " && "

cmd.exe /c $cmd
