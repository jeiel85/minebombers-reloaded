# Packages the Windows release: MineBombers.exe, the SDL2 DLLs, their license texts and the notices, as one zip plus its SHA-256.
#
#   cargo build --release
#   pwsh scripts/package_windows.ps1            # writes dist/MineBombers-<version>-windows-x64.zip
#
# The zip contains no original Mine Bombers files; players bring their own copy.

param(
    [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$name = "MineBombers-$version-windows-x64"

# The DLLs the exe loads at start-up. libgme/libxmp/libopus/... are SDL2_mixer's music decoders.
$dlls = @(
    "SDL2.dll", "SDL2_mixer.dll", "libgme.dll", "libogg-0.dll",
    "libopus-0.dll", "libopusfile-0.dll", "libwavpack-1.dll", "libxmp.dll"
)
$files = @("target/release/MineBombers.exe") + $dlls + @("packaging/windows/README.txt", "packaging/windows/NOTICE.txt")
$licenseDir = "packaging/windows/licenses"
if (-not (Test-Path "$licenseDir/LICENSE.gme.txt")) { throw "Missing $licenseDir (license texts of the bundled DLLs)" }
foreach ($f in $files) {
    if (-not (Test-Path $f)) { throw "Missing $f (run 'cargo build --release' first)" }
}

$stage = Join-Path $OutDir $name
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
foreach ($f in $files) { Copy-Item $f $stage }
Copy-Item $licenseDir (Join-Path $stage "licenses") -Recurse

$zip = Join-Path $OutDir "$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory((Resolve-Path $stage), (Join-Path (Resolve-Path $OutDir) "$name.zip"), [System.IO.Compression.CompressionLevel]::Optimal, $true)

$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
# LF, not the CRLF that Set-Content adds: `sha256sum -c` on Linux/Git Bash rejects a CR at the end of the file name.
[System.IO.File]::WriteAllText((Join-Path (Resolve-Path $OutDir) "$name.zip.sha256"), "$hash  $name.zip`n", [System.Text.Encoding]::ASCII)

Write-Host "$zip"
Write-Host "sha256 $hash"
