@echo off
setlocal enabledelayedexpansion
title Aquerty Stop - Git Push / Release

cd /d "%~dp0"

echo *--- Aquerty Stop ---*
echo.
echo /--- Statut Git ---/
git status -s
echo.

set /p msg="Message de commit : "
if "%msg%"=="" (
    echo [ERREUR] Message vide.
    goto :fin
)

echo.
echo [1/4] git add...
git add -A
if %errorlevel% neq 0 (
    echo [ERREUR] git add a echoue.
    goto :fin
)

echo [2/4] git commit...
git commit -m "%msg%"
if %errorlevel% neq 0 (
    echo [INFO] Rien a committer, ou erreur.
    echo On continue quand meme pour un eventuel tag...
)

echo [3/4] git push...
git push -u origin HEAD
if %errorlevel% neq 0 (
    echo [ERREUR] Push echoue (reseau / conflit / remote).
    goto :fin
)

echo.
set /p doRelease="Creer une release GitHub (tag + Actions) ? (O/N) : "
if /i not "%doRelease%"=="O" goto :ok

REM Propose la version de package.json (ex: 1.1.0)
set "ver="
for /f "usebackq tokens=2 delims=:, " %%A in (`findstr /C:"\"version\"" package.json`) do (
    if not defined ver set "ver=%%~A"
)
set "ver=%ver:"=%"

echo.
echo Version detectee dans package.json : %ver%
echo IMPORTANT : elle doit matcher tauri.conf.json + Cargo.toml
set /p tagVer="Tag a pousser (ex: 1.2.0) [%ver%] : "
if "%tagVer%"=="" set "tagVer=%ver%"

REM Enleve un eventuel v au debut pour uniformiser
if /i "%tagVer:~0,1%"=="v" set "tagVer=%tagVer:~1%"
set "tagName=v%tagVer%"

echo.
echo [4/4] Tag %tagName% + push...
git tag "%tagName%"
if %errorlevel% neq 0 (
    echo [ERREUR] Tag impossible (existe deja ?).
    goto :fin
)

git push origin "%tagName%"
if %errorlevel% neq 0 (
    echo [ERREUR] Push du tag echoue.
    goto :fin
)

echo.
echo Release lancee : regarde l'onglet Actions sur GitHub.
echo https://github.com/STAELH81/aquerty-desktop/actions
goto :ok

:ok
echo.
echo \--- Fini ---/

:fin
echo.
pause
