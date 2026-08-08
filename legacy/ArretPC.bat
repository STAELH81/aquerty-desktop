@echo off
title Arret automatique
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0ArretPC.ps1"

echo.
echo ----------------------------------------
echo Le script est termine.
echo ----------------------------------------
pause