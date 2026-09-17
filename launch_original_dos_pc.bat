@echo off
title Mine Bombers 3.11 (1995 Original PC DOS Edition)
cd /d "%~dp0"

echo ========================================================
echo   MINE BOMBERS 3.11 (1995) - Original PC DOS Edition
echo ========================================================
echo.
echo Checking client build...
if not exist "apps\client\dist\dos.html" (
    echo Building client assets for DOS player...
    call npm --workspace @minebombers/client run build
)

echo Launching original DOS PC emulator window...
node scripts/desktop_launcher.cjs --dos
pause
