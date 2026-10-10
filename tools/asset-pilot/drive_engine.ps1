# Drive a running MineBombers.exe for visual checks: post keys to its window without taking focus (PostMessage),
# then capture the window with PrintWindow. Works while the PC is locked or the window is behind others.
# Needs Windows PowerShell 5.1 (System.Drawing): powershell.exe -File drive_engine.ps1 -Keys "Down,Return" -Out shot.png
# -Keys: comma-separated names from the $vk table below, sent one by one with -Wait ms between them.
param([string]$Keys = "", [string]$Out = "", [int]$Wait = 800, [int]$Hold = 60)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class MbDrive {
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint c, uint t);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  public struct RECT { public int L, T, R, B; }
}
"@
$p = Get-Process MineBombers -ErrorAction Stop | Select-Object -First 1
$h = $p.MainWindowHandle
$vk = @{ Return = 0x0D; Escape = 0x1B; Up = 0x26; Down = 0x28; Left = 0x25; Right = 0x27; Tab = 0x09; Space = 0x20;
         PageDown = 0x22; PageUp = 0x21; F = 0x46; E = 0x45; Q = 0x51 }
$ext = @('Up', 'Down', 'Left', 'Right', 'PageDown', 'PageUp')
foreach ($k in ($Keys -split ',' | Where-Object { $_ })) {
  $v = $vk[$k]; $sc = [MbDrive]::MapVirtualKey($v, 0)
  $e = if ($ext -contains $k) { 1 } else { 0 }
  $down = 1 -bor ($sc -shl 16) -bor ($e -shl 24)
  $up = $down -bor (3 -shl 30)
  [MbDrive]::PostMessage($h, 0x100, [IntPtr]$v, [IntPtr]$down) | Out-Null
  Start-Sleep -Milliseconds $Hold
  [MbDrive]::PostMessage($h, 0x101, [IntPtr]$v, [IntPtr][int64]$up) | Out-Null
  Start-Sleep -Milliseconds $Wait
}
if ($Out) {
  $r = New-Object MbDrive+RECT; [MbDrive]::GetClientRect($h, [ref]$r) | Out-Null
  $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
  $g = [System.Drawing.Graphics]::FromImage($bmp); $dc = $g.GetHdc()
  [MbDrive]::PrintWindow($h, $dc, 3) | Out-Null
  $g.ReleaseHdc($dc); $g.Dispose(); $bmp.Save($Out); $bmp.Dispose()
  "saved $Out"
}
