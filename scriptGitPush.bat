@echo off
setlocal EnableExtensions
title Aquerty Stop - Git Push / Release
cd /d "%~dp0"

echo.
echo  ========================================
echo   AQUERTY STOP - Git Push / Release
echo  ========================================
echo.
echo  Statut Git :
git status -s
echo.

:askMsg
set "msg="
set /p msg=Message de commit : 
if not defined msg (
    echo.
    echo  [ERREUR] Message vide — reessaie.
    echo.
    goto :askMsg
)

echo.
echo  [1/4] git add...
git add -A || (
    echo  [ERREUR] git add a echoue.
    goto :fin
)

echo  [2/4] git commit...
git commit -m "%msg%"
if errorlevel 1 (
    echo  [INFO] Rien de nouveau a committer — on continue.
)

echo  [3/4] git push...
git push -u origin HEAD
if errorlevel 1 (
    echo  [ERREUR] git push a echoue.
    goto :fin
)

echo.
echo  ========================================
echo   RELEASE ?
echo  ========================================
echo   O = creer un tag et lancer GitHub Actions
echo   N = juste push, pas de release
echo.
choice /C ON /N /M "Ton choix [O/N] : "
if errorlevel 2 goto :ok
if errorlevel 1 goto :release
goto :ok

:release
set "ver="
for /f "tokens=2 delims=:, " %%A in ('findstr /C:"\"version\"" package.json') do (
    if not defined ver set "ver=%%~A"
)
set "ver=%ver:"=%"

echo.
echo  Version package.json : %ver%
echo  (doit matcher tauri.conf.json + Cargo.toml)
echo.
set "tagVer="
set /p tagVer=Tag a pousser [%ver%] : 
if not defined tagVer set "tagVer=%ver%"

if /i "%tagVer:~0,1%"=="v" set "tagVer=%tagVer:~1%"
set "tagName=v%tagVer%"

echo.
echo  [4/4] git tag %tagName% ...
git tag "%tagName%"
if errorlevel 1 (
    echo  [ERREUR] Tag impossible — il existe peut-etre deja.
    goto :fin
)

echo  Push du tag...
git push origin "%tagName%"
if errorlevel 1 (
    echo  [ERREUR] Push du tag echoue.
    goto :fin
)

echo.
echo  OK — regarde Actions :
echo  https://github.com/STAELH81/aquerty-desktop/actions
goto :ok

:ok
echo.
echo  --- Fini ---

:fin
echo.
echo  Appuie sur une touche pour fermer...
pause >nul
endlocal
