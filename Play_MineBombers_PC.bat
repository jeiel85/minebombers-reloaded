@echo off
title Mine Bombers 3.11 (1995 Original PC Edition)
cd /d "%~dp0"

echo ========================================================
echo   MINE BOMBERS 3.11 (1995) - Original PC DOS Edition
echo ========================================================
echo.
echo   [ SPEED CONTROLS IN GAME ]
echo   - Ctrl + F11   : Slow Down Speed (-2,000 cycles)
echo   - Ctrl + F12   : Speed Up (+2,000 cycles)
echo   - Alt  + F12   : Turbo Fast-Forward Mode
echo   - Alt  + Enter : Fullscreen Toggle
echo.
echo ========================================================
echo.

if exist "tools\dosbox\dosbox.exe" (
    echo Starting native Windows PC DOSBox emulator...
    start "" "tools\dosbox\dosbox.exe" -conf "minebombers_pc.conf"
    exit /b 0
)

echo Native DOSBox not found in tools\dosbox.
echo Attempting 1-click setup of portable DOSBox for Windows...

if not exist "tools\dosbox" mkdir "tools\dosbox"
echo Downloading portable DOSBox Staging (36MB)...
curl -L -o "%TEMP%\dosbox.zip" https://github.com/dosbox-staging/dosbox-staging/releases/download/v0.83.0/dosbox-staging-windows-x64-v0.83.0.zip

if exist "%TEMP%\dosbox.zip" (
    echo Extracting portable DOSBox...
    tar -xf "%TEMP%\dosbox.zip" --strip-components=1 -C "tools\dosbox"
    del "%TEMP%\dosbox.zip"
    echo Launching Mine Bombers...
    start "" "tools\dosbox\dosbox.exe" -conf "minebombers_pc.conf"
    exit /b 0
)

echo Falling back to desktop web runner...
node scripts\desktop_launcher.cjs --dos
