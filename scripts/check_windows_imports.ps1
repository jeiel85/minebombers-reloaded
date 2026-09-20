# Fails when an exe or DLL imports a DLL that a clean Windows PC would not have.
#
#   pwsh scripts/check_windows_imports.ps1 -Exe dist/MineBombers-0.1.1-windows-x64/MineBombers.exe -Bundled SDL2.dll,SDL2_mixer.dll
#
# An import is fine when it is one of the bundled DLLs, an API-set name (api-ms-win-*), or a DLL in System32.
# The Visual C++ runtime (VCRUNTIME140, MSVCP140, ...) is refused even though the CI runner has it in
# System32: players do not, and MineBombers.exe is built with +crt-static (.cargo/config.toml) so that it does not need it.

param(
    [Parameter(Mandatory)][string]$Exe,
    [string[]]$Bundled = @()
)

$ErrorActionPreference = "Stop"

# The names of the DLLs a PE32+ file imports, at load time and delay-loaded.
function Get-PeImports([string]$Path) {
    $b = [System.IO.File]::ReadAllBytes((Resolve-Path $Path))
    $pe = [BitConverter]::ToInt32($b, 0x3C)
    if ([BitConverter]::ToUInt32($b, $pe) -ne 0x00004550) { throw "$Path is not a PE file" }
    $sections = [BitConverter]::ToUInt16($b, $pe + 6)
    $optional = $pe + 24
    if ([BitConverter]::ToUInt16($b, $optional) -ne 0x20B) { throw "$Path is not a 64-bit (PE32+) file" }
    $table = $optional + [BitConverter]::ToUInt16($b, $pe + 20)

    function ToOffset([uint32]$rva) {
        for ($i = 0; $i -lt $sections; $i++) {
            $s = $table + 40 * $i
            $virtualSize = [BitConverter]::ToUInt32($b, $s + 8)
            $virtualAddress = [BitConverter]::ToUInt32($b, $s + 12)
            $rawSize = [BitConverter]::ToUInt32($b, $s + 16)
            $rawPointer = [BitConverter]::ToUInt32($b, $s + 20)
            if ($rva -ge $virtualAddress -and $rva -lt $virtualAddress + [Math]::Max($virtualSize, $rawSize)) {
                return [int]($rawPointer + $rva - $virtualAddress)
            }
        }
        throw "RVA $rva is in no section of $Path"
    }
    function ReadName([uint32]$rva) {
        $o = ToOffset $rva
        $e = $o
        while ($b[$e] -ne 0) { $e++ }
        return [System.Text.Encoding]::ASCII.GetString($b, $o, $e - $o)
    }

    # data directory 1 = import table (20-byte entries, name at +12), 13 = delay-load imports (32-byte entries, name at +4)
    foreach ($dir in @(@{ Index = 1; Size = 20; Name = 12 }, @{ Index = 13; Size = 32; Name = 4 })) {
        $rva = [BitConverter]::ToUInt32($b, $optional + 112 + 8 * $dir.Index)
        if ($rva -eq 0) { continue }
        $at = ToOffset $rva
        while ($true) {
            $nameRva = [BitConverter]::ToUInt32($b, $at + $dir.Name)
            if ($nameRva -eq 0) { break }
            ReadName $nameRva
            $at += $dir.Size
        }
    }
}

$imports = @(Get-PeImports $Exe | Sort-Object -Unique)
$system = Join-Path $env:windir "System32"
$bad = @()
foreach ($name in $imports) {
    $lower = $name.ToLower()
    if ($lower -match '^(vcruntime|msvcp|msvcr\d|concrt|vcomp|vccorlib)') {
        $bad += "$name (Visual C++ runtime, not on a clean PC)"
    } elseif ($Bundled -contains $name -or ($Bundled | Where-Object { $_ -ieq $name })) {
        continue
    } elseif ($lower -like 'api-ms-win-*' -or $lower -like 'ext-ms-win-*') {
        continue
    } elseif (-not (Test-Path (Join-Path $system $name))) {
        $bad += "$name (neither bundled nor in System32)"
    }
}

Write-Host "$Exe imports: $($imports -join ', ')"
if ($bad.Count -gt 0) {
    throw "$Exe needs DLLs a clean Windows PC may not have:`n  " + ($bad -join "`n  ")
}
Write-Host "OK: every import is bundled or part of Windows."
