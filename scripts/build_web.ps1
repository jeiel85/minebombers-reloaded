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

$wasmSize = (Get-Item ".\web\pkg\mb_wasm.wasm").Length / 1KB
Write-Host "[+] WebAssembly binary built successfully: web\pkg\mb_wasm.wasm ($([Math]::Round($wasmSize, 1)) KB)" -ForegroundColor Green
Write-Host "[*] To test locally in browser, run: npx serve web" -ForegroundColor Cyan
