Write-Host "=========================================" -ForegroundColor Yellow
Write-Host " Building Mine Bombers WebAssembly (WASM)" -ForegroundColor Yellow
Write-Host "=========================================" -ForegroundColor Yellow

$target = "wasm32-unknown-unknown"
cargo build -p mb-wasm --target $target --release

if ($LASTEXITCODE -ne 0) {
    Write-Host "[-] Cargo build failed!" -ForegroundColor Red
    exit $LASTEXITCODE
}

New-Item -ItemType Directory -Path "web\pkg" -Force | Out-Null
Copy-Item ".\target\$target\release\mb_wasm.wasm" ".\web\pkg\mb_wasm.wasm" -Force

# Optional: if MB_GAME_DIR points at your own copy of Mine Bombers 3.11, stage it next to the page so
# it starts without asking for the files. The repository contains no original game files.
if ($env:MB_GAME_DIR) {
    node .\scripts\stage_web_data.mjs
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[-] Staging the game files from MB_GAME_DIR failed!" -ForegroundColor Red
        exit $LASTEXITCODE
    }
} else {
    Write-Host "[*] MB_GAME_DIR is not set: the page will ask for your Mine Bombers 3.11 files." -ForegroundColor Cyan
}

$wasmSize = (Get-Item ".\web\pkg\mb_wasm.wasm").Length / 1KB
Write-Host "[+] WebAssembly binary built successfully: web\pkg\mb_wasm.wasm ($([Math]::Round($wasmSize, 1)) KB)" -ForegroundColor Green
Write-Host "[*] To test locally in browser, run: npx serve web" -ForegroundColor Cyan
