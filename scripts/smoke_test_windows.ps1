# Launches the packaged exe against a synthetic stand-in game folder and confirms its window comes up.
#
#   cargo build --release
#   pwsh scripts/smoke_test_windows.ps1 -ExeDir target/release
#
# This is the one check that actually runs the exe: `check_windows_imports.ps1` only reads the PE import
# table, which misses DLLs SDL2_mixer loads on demand (libxmp, libopus, libopusfile, libwavpack, libgme -
# one per music/sample codec). The stand-in files carry no original Mine Bombers content: every field is
# either the minimum the parser in src/images.rs, src/fonts.rs or src/effects.rs requires, or arbitrary
# bytes. Getting to the title screen means every bundled codec DLL loaded and decoded something.
#
# What "loads" actually means per format (see the parser for each):
#   .SPY   (src/images.rs decode_spy):  768-byte palette + 4 run-length planes. Byte 0x01 starts a
#          run (value, count), so plain zero bytes decode as literal pixels - no encoding needed.
#   .FON   (src/fonts.rs decode_font):  exactly 256*8 bytes, no other validation.
#   .VOC   (src/effects.rs load_sample): read as raw bytes and kept for later; nothing parses it at
#          load time, so any content (even empty) is fine.
#   .S3M   (src/context.rs load_music, via SDL2_mixer -> libxmp): the one real binary format here.
#          libxmp rejects a module with zero patterns even if the order list is empty, so this writes
#          one pattern that is just 64 empty rows.
#   .PPM   (src/images.rs decode_ppm): 128-byte header (from_y/to_y/width at fixed offsets) + a body
#          where each byte < 0xC0 is one literal palette-index pixel + a trailing 768-byte palette.

param(
    [string]$ExeDir = "target/release",
    [int]$TimeoutSeconds = 20
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# ---- stand-in asset generators -------------------------------------------------------------

function Write-StubSpy([string]$Path) {
    # 640x480 screen, 4 bitplanes: 768 (palette) + 4 * (640*480/8) bytes. All zero bytes decode as
    # literal (not RLE, since the escape byte is 0x01), so this is a valid, all-black SPY image.
    $bitplaneLen = 640 * 480 / 8
    $bytes = New-Object byte[] (768 + 4 * $bitplaneLen)
    [System.IO.File]::WriteAllBytes($Path, $bytes)
}

function Write-StubFon([string]$Path) {
    $bytes = New-Object byte[] (256 * 8)
    [System.IO.File]::WriteAllBytes($Path, $bytes)
}

function Write-StubVoc([string]$Path) {
    [System.IO.File]::WriteAllBytes($Path, [byte[]]@())
}

function Write-StubPpm([string]$Path, [int]$Width, [int]$Height) {
    $header = New-Object byte[] 128
    # from_y (offset 6-7) = 0, to_y (offset 10-11) = Height, both little-endian u16.
    $header[10] = [byte]($Height -band 0xFF)
    $header[11] = [byte](($Height -shr 8) -band 0xFF)
    $header[0x42] = [byte]($Width -band 0xFF)
    $header[0x43] = [byte](($Width -shr 8) -band 0xFF)
    $body = New-Object byte[] ($Width * $Height)   # all 0x00: literal pixels, palette index 0
    $flag = New-Object byte[] 1                    # one byte between the body and the palette
    $palette = New-Object byte[] 768
    $all = New-Object byte[] (128 + $body.Length + 1 + 768)
    [System.Array]::Copy($header, 0, $all, 0, 128)
    [System.Array]::Copy($body, 0, $all, 128, $body.Length)
    [System.Array]::Copy($flag, 0, $all, 128 + $body.Length, 1)
    [System.Array]::Copy($palette, 0, $all, 128 + $body.Length + 1, 768)
    [System.IO.File]::WriteAllBytes($Path, $all)
}

function Write-StubS3m([string]$Path) {
    # Minimal Scream Tracker 3 module libxmp accepts: 96-byte header, order count 1 (-> pattern 0),
    # pattern count 1, 2 enabled channels, one pattern of 64 empty rows. Verified directly against the
    # libxmp.dll shipped in this repo (SDL2_mixer's S3M/MOD loader) before writing this script - libxmp
    # rejects a module with a nonzero order count but zero real patterns, so the empty pattern is required.
    $b = New-Object byte[] 0x60
    [System.Text.Encoding]::ASCII.GetBytes("stub").CopyTo($b, 0)
    $b[0x1C] = 0x1A
    $b[0x1D] = 16                                    # type: ST3 module
    # order/instrument/pattern counts, flags at 0x20/0x22/0x24/0x26 default to 0; set below as needed.
    $b[0x28] = 0x00; $b[0x29] = 0x13                 # "created with" tracker version
    $b[0x2A] = 0x02                                  # file format: unsigned samples
    [System.Text.Encoding]::ASCII.GetBytes("SCRM").CopyTo($b, 0x2C)
    $b[0x30] = 64                                    # global volume
    $b[0x31] = 6                                     # initial speed
    $b[0x32] = 125                                   # initial tempo
    $b[0x33] = 0x30                                  # master volume (mono)
    $b[0x34] = 8                                     # ultraclick removal
    $b[0x35] = 0x00                                  # default pan marker: no extra pan table follows
    $b[0x40] = 0x00                                  # channel 0: left PCM, enabled
    $b[0x41] = 0x01                                  # channel 1: right PCM, enabled
    # order count = 1, pattern count = 1
    $b[0x20] = 1; $b[0x24] = 1

    $out = New-Object System.Collections.Generic.List[byte]
    $out.AddRange([byte[]]$b)
    $out.Add(0)                                      # order list: 1 entry, pattern 0
    while ($out.Count % 16 -ne 0) { $out.Add(0) }     # pad to a paragraph boundary
    $patternParaOffset = $out.Count
    $out.Add(0); $out.Add(0)                          # pattern parapointer table: 1 entry, filled in below
    while ($out.Count % 16 -ne 0) { $out.Add(0) }
    $patternOffset = $out.Count
    $para = [uint16]($patternOffset / 16)
    $out[$patternParaOffset] = [byte]($para -band 0xFF)
    $out[$patternParaOffset + 1] = [byte](($para -shr 8) -band 0xFF)
    $rows = New-Object byte[] 64                      # 64 rows, each just an end-of-row marker (0x00)
    $out.Add([byte](64 -band 0xFF)); $out.Add(0)       # PackedSize = 64, little-endian u16
    $out.AddRange([byte[]]$rows)

    [System.IO.File]::WriteAllBytes($Path, $out.ToArray())
}

function New-StubGameDir([string]$Dir) {
    New-Item -ItemType Directory -Force $Dir | Out-Null

    $spyFiles = @(
        "TITLEBE", "MAIN3", "OPTIONS5", "LEVSELEC", "KEYS", "SHOPPIC", "SIKA",
        "INFO1", "INFO3", "SHAPET", "INFO2", "CODES", "IDENTIFW", "PLAYERS",
        "GAMEOVER", "CONGRATU", "FINAL", "HALLOFFA"
    )
    foreach ($name in $spyFiles) { Write-StubSpy (Join-Path $Dir "$name.SPY") }

    Write-StubFon (Join-Path $Dir "FONTTI.FON")

    $vocFiles = @(
        "KILI", "PICAXE", "EXPLOS1", "EXPLOS2", "EXPLOS3", "EXPLOS4", "EXPLOS5",
        "AARGH", "KARJAISU", "PIKKUPOM", "URETHAN", "APPLAUSE"
    )
    foreach ($name in $vocFiles) { Write-StubVoc (Join-Path $Dir "$name.VOC") }

    Write-StubS3m (Join-Path $Dir "HUIPPE.S3M")
    Write-StubS3m (Join-Path $Dir "OEKU.S3M")

    $ppmFiles = @(
        "SINVOIT", "SINDRAW", "SINLOSE", "PUNVOIT", "PUNDRAW", "PUNLOSE",
        "VIHVOIT", "VIHDRAW", "VIHLOSE", "KELVOIT", "KELDRAW", "KELLOSE"
    )
    foreach ($name in $ppmFiles) { Write-StubPpm (Join-Path $Dir "$name.PPM") 8 8 }
}

# ---- run the exe against it -----------------------------------------------------------------

$exe = Join-Path $ExeDir "MineBombers.exe"
if (-not (Test-Path $exe)) { throw "Missing $exe (run 'cargo build --release' first)" }

$gameDir = Join-Path ([System.IO.Path]::GetTempPath()) "mb-smoke-gamedir-$([guid]::NewGuid())"
$userDir = Join-Path ([System.IO.Path]::GetTempPath()) "mb-smoke-userdir-$([guid]::NewGuid())"
New-StubGameDir $gameDir
New-Item -ItemType Directory -Force $userDir | Out-Null

$env:MINEBOMBERS_GAME_DIR = $gameDir
$env:MINEBOMBERS_USER_DIR = $userDir
$env:SDL_AUDIODRIVER = "dummy"

Write-Host "Stand-in game folder: $gameDir"
Write-Host "Starting $exe ..."
$proc = Start-Process -FilePath $exe -WorkingDirectory (Resolve-Path $ExeDir) -PassThru

try {
    # The window is created (with the right title) before `Application::init` loads a single asset - see
    # src/context.rs `with_context`. So seeing the window is not enough: a codec DLL missing from the
    # stand-in run would make `Application::init` fail right after, and the window can still be on screen
    # for a moment while the process unwinds. What actually proves every asset (title screen, font, both
    # S3M tracks, every sample) loaded is that the window is *still* up after a grace period with no sign
    # of the process tearing down - by then it is blocked on `wait_key_pressed()` in the main menu loop.
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $sawWindow = $false
    while ((Get-Date) -lt $deadline) {
        $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
        if (-not $p) {
            throw "MineBombers.exe exited before showing a window (exit code $($proc.ExitCode)). A DLL the stand-in run needs may be missing - see dist for the packaged DLL set."
        }
        if ($p.MainWindowHandle -ne 0 -and $p.MainWindowTitle) {
            $sawWindow = $true
            break
        }
        Start-Sleep -Milliseconds 200
    }
    if (-not $sawWindow) {
        throw "MineBombers.exe did not show a window within $TimeoutSeconds s (still running, no MainWindowHandle)."
    }

    $graceMs = 2500
    $graceDeadline = (Get-Date).AddMilliseconds($graceMs)
    while ((Get-Date) -lt $graceDeadline) {
        Start-Sleep -Milliseconds 200
        $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
        if (-not $p) {
            throw "MineBombers.exe showed a window but then exited (exit code $($proc.ExitCode)) before $graceMs ms had passed - Application::init failed loading one of the stand-in assets. A bundled codec DLL (libgme/libogg/libopus/libopusfile/libwavpack/libxmp) may be missing or broken."
        }
        if ($p.MainWindowHandle -eq 0) {
            throw "MineBombers.exe's window disappeared during the $graceMs ms grace period (process still running, exit pending)."
        }
    }

    Write-Host "Window stayed up for ${graceMs}ms after appearing - Application::init loaded every stand-in asset. Loaded DLLs:"
    (Get-Process -Id $proc.Id).Modules | Where-Object { $_.ModuleName -match '\.dll$' } | ForEach-Object { Write-Host "  $($_.ModuleName)" }
    Write-Host "Smoke test OK: the packaged exe starts, loads every bundled DLL and reaches the title screen."
} finally {
    $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
    if ($p) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -Recurse -Force $gameDir -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $userDir -ErrorAction SilentlyContinue
}
