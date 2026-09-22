# Launches the built exe against a synthetic stand-in game folder and confirms it actually runs.
# Cross-platform (Windows and Linux): CI runs it on both via the `native` matrix job / release.yml.
#
#   cargo build --release
#   pwsh scripts/smoke_test.ps1 -ExeDir target/release
#
# On Windows this is the one check that actually runs the exe: `check_windows_imports.ps1` only reads
# the PE import table, which misses DLLs SDL2_mixer loads on demand (libxmp, libopus, libopusfile,
# libwavpack, libgme - one per music/sample codec). On Linux there is no equivalent static check at all
# yet (SDL2/SDL2_mixer come from apt, not bundled), so this is the only runtime verification either way.
# The stand-in files carry no original Mine Bombers content: every field is either the minimum the
# parser in src/images.rs, src/fonts.rs or src/effects.rs requires, or arbitrary bytes. Reaching the
# title screen means every codec needed to decode them actually loaded.
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
#
# Windows has a real desktop session, so we wait for the actual window and then confirm it stays up
# (see the comment further down for why "stays up" and not just "appears"). Linux CI has no display, so
# this runs SDL on its `dummy` video driver instead - there is no window to inspect, so the check is
# simply "the process is still running (i.e. still blocked in the menu's event loop) after a grace
# period", which the same underlying risk (Application::init failing partway through) still falsifies.

param(
    [string]$ExeDir = "target/release",
    [int]$TimeoutSeconds = 20
)

$onWindows = $IsWindows -or ($null -eq $IsWindows) # $IsWindows only exists on pwsh 6+; Windows PowerShell 5.1 is always Windows

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

function Write-StubS3m([string]$Path, [int]$InstrumentCount) {
    # Minimal Scream Tracker 3 module: 96-byte header, order count 1 (-> pattern 0), pattern count 1,
    # 2 enabled channels, one pattern of 64 empty rows, plus $InstrumentCount empty (type 0) instruments.
    #
    # $InstrumentCount has to differ by platform - verified directly against the two DLLs/shared
    # libraries SDL2_mixer's S3M/MOD loader actually resolves to, by probing several candidates with
    # sdl2::mixer::Music::from_file and seeing which loaded (see PR history for the probe):
    #   Windows bundles libxmp.dll:        rejects the file if InstrumentCount > 0, however well-formed
    #                                       the instrument entries are (libxmp wants a bare 0-instrument
    #                                       module here) -> pass 0.
    #   Linux's apt libsdl2-mixer-dev uses ModPlug (linked into SDL2_mixer itself): rejects the file if
    #                                       InstrumentCount = 0, but accepts an all-zero (type 0, "empty")
    #                                       instrument entry happily -> pass 1.
    # There is no single byte layout both accept; this is an actual difference between the two codec
    # libraries, not a bug to keep chasing.
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
    # order count = 1, instrument count as requested, pattern count = 1
    $b[0x20] = 1
    $b[0x22] = [byte]($InstrumentCount -band 0xFF)
    $b[0x24] = 1

    $out = New-Object System.Collections.Generic.List[byte]
    $out.AddRange([byte[]]$b)
    $out.Add(0)                                      # order list: 1 entry, pattern 0
    while ($out.Count % 16 -ne 0) { $out.Add(0) }     # pad to a paragraph boundary

    $instrumentParaOffset = $out.Count
    for ($i = 0; $i -lt $InstrumentCount; $i++) { $out.Add(0); $out.Add(0) } # instrument parapointer table

    $patternParaOffset = $out.Count
    $out.Add(0); $out.Add(0)                          # pattern parapointer table: 1 entry, filled in below
    while ($out.Count % 16 -ne 0) { $out.Add(0) }

    for ($i = 0; $i -lt $InstrumentCount; $i++) {
        while ($out.Count % 16 -ne 0) { $out.Add(0) }
        $para = [uint16]($out.Count / 16)
        $entryOffset = $instrumentParaOffset + $i * 2
        $out[$entryOffset] = [byte]($para -band 0xFF)
        $out[$entryOffset + 1] = [byte](($para -shr 8) -band 0xFF)
        for ($j = 0; $j -lt 80; $j++) { $out.Add(0) }  # type 0 = empty instrument; rest is unused
    }

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

    $s3mInstrumentCount = if ($onWindows) { 0 } else { 1 } # see Write-StubS3m: libxmp vs ModPlug disagree
    Write-StubS3m (Join-Path $Dir "HUIPPE.S3M") $s3mInstrumentCount
    Write-StubS3m (Join-Path $Dir "OEKU.S3M") $s3mInstrumentCount

    $ppmFiles = @(
        "SINVOIT", "SINDRAW", "SINLOSE", "PUNVOIT", "PUNDRAW", "PUNLOSE",
        "VIHVOIT", "VIHDRAW", "VIHLOSE", "KELVOIT", "KELDRAW", "KELLOSE"
    )
    foreach ($name in $ppmFiles) { Write-StubPpm (Join-Path $Dir "$name.PPM") 8 8 }
}

# ---- run the exe against it -----------------------------------------------------------------

$exeName = if ($onWindows) { "MineBombers.exe" } else { "MineBombers" }
$exe = Join-Path $ExeDir $exeName
if (-not (Test-Path $exe)) { throw "Missing $exe (run 'cargo build --release' first)" }

$gameDir = Join-Path ([System.IO.Path]::GetTempPath()) "mb-smoke-gamedir-$([guid]::NewGuid())"
$userDir = Join-Path ([System.IO.Path]::GetTempPath()) "mb-smoke-userdir-$([guid]::NewGuid())"
New-StubGameDir $gameDir
New-Item -ItemType Directory -Force $userDir | Out-Null

$env:MINEBOMBERS_GAME_DIR = $gameDir
$env:MINEBOMBERS_USER_DIR = $userDir
$env:SDL_AUDIODRIVER = "dummy"
if (-not $onWindows) {
    $env:SDL_VIDEODRIVER = "dummy" # CI has no display; Windows runners have a real desktop session
}

Write-Host "Stand-in game folder: $gameDir"
Write-Host "Starting $exe ..."
$proc = Start-Process -FilePath $exe -WorkingDirectory (Resolve-Path $ExeDir) -PassThru

try {
    if ($onWindows) {
        # The window is created (with the right title) before `Application::init` loads a single asset -
        # see src/context.rs `with_context`. So seeing the window is not enough: a codec DLL missing from
        # the stand-in run would make `Application::init` fail right after, and the window can still be on
        # screen for a moment while the process unwinds. What actually proves every asset (title screen,
        # font, both S3M tracks, every sample) loaded is that the window is *still* up after a grace period
        # with no sign of tearing down - by then it is blocked on `wait_key_pressed()` in the menu loop.
        $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
        $sawWindow = $false
        while ((Get-Date) -lt $deadline) {
            $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
            if (-not $p) {
                throw "$exeName exited before showing a window (exit code $($proc.ExitCode)). A DLL the stand-in run needs may be missing."
            }
            if ($p.MainWindowHandle -ne 0 -and $p.MainWindowTitle) {
                $sawWindow = $true
                break
            }
            Start-Sleep -Milliseconds 200
        }
        if (-not $sawWindow) {
            throw "$exeName did not show a window within $TimeoutSeconds s (still running, no MainWindowHandle)."
        }

        $graceMs = 2500
        $graceDeadline = (Get-Date).AddMilliseconds($graceMs)
        while ((Get-Date) -lt $graceDeadline) {
            Start-Sleep -Milliseconds 200
            $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
            if (-not $p) {
                throw "$exeName showed a window but then exited (exit code $($proc.ExitCode)) before $graceMs ms had passed - Application::init failed loading one of the stand-in assets. A codec DLL (libgme/libogg/libopus/libopusfile/libwavpack/libxmp) may be missing or broken."
            }
            if ($p.MainWindowHandle -eq 0) {
                throw "$exeName's window disappeared during the $graceMs ms grace period (process still running, exit pending)."
            }
        }

        Write-Host "Window stayed up for ${graceMs}ms after appearing - Application::init loaded every stand-in asset. Loaded DLLs:"
        (Get-Process -Id $proc.Id).Modules | Where-Object { $_.ModuleName -match '\.dll$' } | ForEach-Object { Write-Host "  $($_.ModuleName)" }
    } else {
        # No display, so SDL runs on the `dummy` video driver and there is no window to inspect. The same
        # risk (Application::init failing partway through the stand-in assets) still shows up as an early
        # exit, so "still running after a grace period" (blocked on wait_key_pressed) is the whole check.
        $graceMs = 2500
        $graceDeadline = (Get-Date).AddMilliseconds($graceMs)
        while ((Get-Date) -lt $graceDeadline) {
            Start-Sleep -Milliseconds 200
            $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
            if (-not $p) {
                throw "$exeName exited (exit code $($proc.ExitCode)) before $graceMs ms had passed - Application::init failed loading one of the stand-in assets, or a shared library SDL2_mixer needs on demand is missing."
            }
        }
        Write-Host "Process stayed alive for ${graceMs}ms (SDL_VIDEODRIVER=dummy) - Application::init loaded every stand-in asset and is blocked in the menu's event loop."
    }
    Write-Host "Smoke test OK: the exe starts, loads every codec it needs and reaches the title screen."
} finally {
    $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
    if ($p) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -Recurse -Force $gameDir -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $userDir -ErrorAction SilentlyContinue
}
