[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string[]]$Path
)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'PE hardening verification requires Windows.' }

$IMAGE_FILE_MACHINE_AMD64 = 0x8664
$IMAGE_NT_OPTIONAL_HDR64_MAGIC = 0x20B
$IMAGE_DLLCHARACTERISTICS_HIGH_ENTROPY_VA = 0x0020
$IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE = 0x0040
$IMAGE_DLLCHARACTERISTICS_NX_COMPAT = 0x0100

function Read-U16([byte[]]$Bytes,[int]$Offset) {
    if ($Offset -lt 0 -or $Offset + 2 -gt $Bytes.Length) { throw "PE field exceeds file bounds at offset $Offset." }
    return [BitConverter]::ToUInt16($Bytes,$Offset)
}
function Read-U32([byte[]]$Bytes,[int]$Offset) {
    if ($Offset -lt 0 -or $Offset + 4 -gt $Bytes.Length) { throw "PE field exceeds file bounds at offset $Offset." }
    return [BitConverter]::ToUInt32($Bytes,$Offset)
}

foreach ($candidate in $Path) {
    $resolved = (Resolve-Path $candidate).Path
    $bytes = [IO.File]::ReadAllBytes($resolved)
    if ($bytes.Length -lt 0x100) { throw "PE image is implausibly small: $resolved" }
    if ($bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) { throw "DOS MZ signature is missing: $resolved" }

    $peOffset = [int](Read-U32 $bytes 0x3C)
    if ($peOffset -lt 0x40 -or $peOffset + 24 -gt $bytes.Length) { throw "PE header offset is invalid: $resolved" }
    if ($bytes[$peOffset] -ne 0x50 -or $bytes[$peOffset+1] -ne 0x45 -or $bytes[$peOffset+2] -ne 0 -or $bytes[$peOffset+3] -ne 0) {
        throw "PE signature is invalid: $resolved"
    }

    $coff = $peOffset + 4
    $machine = Read-U16 $bytes $coff
    if ($machine -ne $IMAGE_FILE_MACHINE_AMD64) {
        throw ("Production payload must be AMD64; {0} machine=0x{1:X4}" -f $resolved,$machine)
    }
    $optionalSize = Read-U16 $bytes ($coff + 16)
    $optional = $coff + 20
    if ($optionalSize -lt 0x48 -or $optional + $optionalSize -gt $bytes.Length) {
        throw "PE optional header is truncated: $resolved"
    }
    $magic = Read-U16 $bytes $optional
    if ($magic -ne $IMAGE_NT_OPTIONAL_HDR64_MAGIC) {
        throw ("Production payload must be PE32+; {0} optional magic=0x{1:X4}" -f $resolved,$magic)
    }

    # DllCharacteristics is at offset 0x46 in both PE32 and PE32+ optional headers.
    $dllCharacteristics = Read-U16 $bytes ($optional + 0x46)
    $required = [ordered]@{
        HIGH_ENTROPY_VA = $IMAGE_DLLCHARACTERISTICS_HIGH_ENTROPY_VA
        DYNAMIC_BASE    = $IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE
        NX_COMPAT       = $IMAGE_DLLCHARACTERISTICS_NX_COMPAT
    }
    $missing = @()
    foreach ($entry in $required.GetEnumerator()) {
        if (($dllCharacteristics -band [int]$entry.Value) -eq 0) { $missing += $entry.Key }
    }
    if ($missing.Count -gt 0) {
        throw ("PE hardening flags missing from {0}: {1} (DllCharacteristics=0x{2:X4})" -f $resolved,($missing -join ', '),$dllCharacteristics)
    }
    Write-Host ("PE hardening OK: {0} [DllCharacteristics=0x{1:X4}]" -f (Split-Path $resolved -Leaf),$dllCharacteristics) -ForegroundColor Green
}
