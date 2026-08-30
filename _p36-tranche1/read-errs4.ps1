param([string]$LogPath = 'C:\AetherCore-P36\incoming\tranche1-rawwire-build.log')
$lines = Get-Content -LiteralPath $LogPath -Encoding Unicode
$out = @()
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^error') {
        $out += $lines[$i]
        if ($i + 1 -lt $lines.Count) { $out += $lines[$i + 1] }
    }
}
$out | Set-Content 'C:\Users\Public\errs-out.txt' -Encoding ascii
Get-Content 'C:\Users\Public\errs.txt' -ErrorAction SilentlyContinue
