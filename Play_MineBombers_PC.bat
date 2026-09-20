@echo off
title Mine Bombers (Native PC Engine)
cd /d "%~dp0"

echo ========================================================
echo   MINE BOMBERS - Pure Native PC Windows Edition (Rust)
echo   [ No DOSBox Emulation / 60 FPS Native Window ]
echo ========================================================
echo.
echo   [ CONTROLS ^& SHORTCUTS ]
echo   - [ or -          : Slow Down Game Speed (-0.25x)
echo   - ] or +          : Speed Up Game Speed (+0.25x)
echo   - Backspace or 0  : Reset Game Speed to 1.00x Normal
echo   - Tab or B        : Toggle Human / CPU Bot in Player Selection
echo   - F5              : Toggle Music On / Off
echo   - F10             : Quit Game / Menu Exit
echo.
echo ========================================================
echo.

if exist "MineBombers.exe" (
    echo Launching optimized native binary...
    start "" "MineBombers.exe" %*
) else if exist "target\release\MineBombers.exe" (
    echo Launching optimized native binary...
    start "" "target\release\MineBombers.exe" %*
) else (
    echo Compiling and running native engine...
    cargo run --release -- %*
)
