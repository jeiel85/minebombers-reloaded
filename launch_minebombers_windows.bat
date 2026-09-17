@echo off
title Mine Bombers - Modern Windows Edition
cd /d "%~dp0"

echo ========================================================
echo   MINE BOMBERS (1995) - Modern Windows Desktop Port
echo ========================================================
echo.
echo Checking client build...
if not exist "apps\client\dist\index.html" (
    echo Building client assets for offline play...
    call npm --workspace @minebombers/client run build
)

echo Launching standalone desktop window...
node scripts/desktop_launcher.cjs
pause
