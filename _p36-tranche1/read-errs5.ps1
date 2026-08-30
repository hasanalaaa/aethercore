param([string]$LogPath = 'C:\AetherCore-P36\incoming\tranche1-rawwire-build.log')
$lines = Get-Content -LiteralPath $LogPath -Encoding Unicode
$hits = @()
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^error') {
        $hits += $lines[$i]
        if ($i + 1 -lt $lines.Count) { $hits += $lines[$i + 1] }
    }
}
$hits | Set-Content 'C:\Users\Public\errs-out.txt' -Encoding ascii
