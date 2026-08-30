param([string]$LogPath = 'C:\AetherCore-P36\incoming\tranche1-rawwire-build.log')
$lines = Get-Content -LiteralPath $LogPath
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^error' -or $lines[$i] -match 'E0\d+:') {
        Write-Output $lines[$i]
        if ($i + 1 -lt $lines.Count) { Write-Output $lines[$i + 1] }
        if ($i + 2 -lt $lines.Count) { Write-Output $lines[$i + 2] }
    }
}
