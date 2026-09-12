#Requires -Version 5.1
<#
.SYNOPSIS
    Capture every screen the installer shows, for DBT-P49-002 / P55 Item 4.D.

.DESCRIPTION
    Launches the bundle and photographs its window on a fixed cadence, saving a
    PNG whenever the window's pixels change. The owner has never seen this
    installer; this produces the evidence rather than a description of it.

    Captures the WINDOW, not the whole desktop, so nothing else on the machine
    is photographed.

    This script only captures. It does not click, accept, or dismiss anything --
    the operator drives the installer. A capture run that advanced the installer
    on its own would be making install decisions unattended, which is not this
    script's job.

.PARAMETER BundlePath
    AetherCoreSetup.exe to launch. If omitted, nothing is launched and the
    script attaches to an installer window that is already open.

.PARAMETER OutputDir
    Where the PNGs go.

.PARAMETER DurationSeconds
    How long to keep watching.

.PARAMETER Locale
    Recorded in the filenames so an English pass and an Arabic pass do not
    overwrite each other.
#>
[CmdletBinding()]
param(
    [string]$BundlePath,
    [string]$OutputDir = (Join-Path $PSScriptRoot '..\out\installer-screens'),
    [int]$DurationSeconds = 240,
    [int]$IntervalMs = 700,
    [string]$Locale = 'en',
    [string]$WindowTitleLike = '*AetherCore*'
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Win32Cap {
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int c);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
}
'@

function Get-WindowRect([IntPtr]$h) {
    $r = New-Object Win32Cap+RECT
    # DWMWA_EXTENDED_FRAME_BOUNDS = 9. GetWindowRect includes the invisible
    # resize border on modern Windows, which puts a transparent margin around
    # every capture; the DWM bounds are what the user actually sees.
    if ([Win32Cap]::DwmGetWindowAttribute($h, 9, [ref]$r, 16) -ne 0) {
        [void][Win32Cap]::GetWindowRect($h, [ref]$r)
    }
    return $r
}

function Get-WindowTitle([IntPtr]$h) {
    $sb = New-Object System.Text.StringBuilder 512
    [void][Win32Cap]::GetWindowTextW($h, $sb, $sb.Capacity)
    return $sb.ToString()
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$stamp = (Get-Date).ToString('yyyyMMdd-HHmmss')

if ($BundlePath) {
    if (-not (Test-Path $BundlePath)) { throw "Bundle not found: $BundlePath" }
    $f = Get-Item $BundlePath
    $sha = (Get-FileHash $BundlePath -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Host "launching $($f.FullName)"
    Write-Host "  $($f.Length) bytes  sha256 $sha"
    # Recorded beside the screenshots so the images are tied to an artefact.
    @{ bundle = $f.FullName; bytes = $f.Length; sha256 = $sha; locale = $Locale; started = $stamp } |
        ConvertTo-Json | Set-Content (Join-Path $OutputDir "capture-$Locale-$stamp.json") -Encoding utf8
    Start-Process $BundlePath | Out-Null
}

Write-Host "watching for $DurationSeconds s. Drive the installer; this only captures."

$deadline = (Get-Date).AddSeconds($DurationSeconds)
$lastHash = ''
$n = 0
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds $IntervalMs
    # Find the target by window handle, not by focus. GetForegroundWindow only
    # works when the installer happens to be focused, which it is not when this
    # script is driven from another console -- the first self-test captured zero
    # frames for exactly that reason.
    $h = [IntPtr]::Zero
    $title = ''
    foreach ($p in (Get-Process -ErrorAction SilentlyContinue |
                    Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle })) {
        if ($p.MainWindowTitle -like $WindowTitleLike) {
            $h = $p.MainWindowHandle
            $title = $p.MainWindowTitle
            break
        }
    }
    if ($h -eq [IntPtr]::Zero -or -not [Win32Cap]::IsWindowVisible($h)) { continue }

    $r = Get-WindowRect $h
    $w = $r.Right - $r.Left; $ht = $r.Bottom - $r.Top
    if ($w -le 0 -or $ht -le 0) { continue }

    $bmp = New-Object System.Drawing.Bitmap $w, $ht
    try {
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        try { $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $ht)) }
        finally { $g.Dispose() }

        # Only save when the pixels actually changed, so a 30-second progress
        # screen yields the frames where it moved rather than 40 identical PNGs.
        $ms = New-Object System.IO.MemoryStream
        try {
            $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
            $bytes = $ms.ToArray()
        } finally { $ms.Dispose() }
        $sha = [BitConverter]::ToString(
            [Security.Cryptography.SHA256]::Create().ComputeHash($bytes)).Replace('-','')
        if ($sha -eq $lastHash) { continue }
        $lastHash = $sha

        $n++
        # Strip only what a filename cannot carry, rather than everything
        # non-ASCII: this machine runs an Arabic UI, and an Arabic window title
        # reduced to the empty string under a [^A-Za-z0-9] filter, producing
        # names like "en-...-001-.png" that say nothing about the screen.
        $safeTitle = ($title -replace '[\\/:*?"<>|]', '-').Trim() -replace '\s+', '-'
        if (-not $safeTitle) { $safeTitle = 'window' }
        if ($safeTitle.Length -gt 60) { $safeTitle = $safeTitle.Substring(0, 60) }
        $name = '{0}-{1}-{2:d3}-{3}.png' -f $Locale, $stamp, $n, $safeTitle
        [IO.File]::WriteAllBytes((Join-Path $OutputDir $name), $bytes)
        Write-Host ("  [{0:d3}] {1}x{2}  {3}" -f $n, $w, $ht, $title)
    } finally { $bmp.Dispose() }
}

Write-Host "`ncaptured $n frame(s) into $OutputDir"
