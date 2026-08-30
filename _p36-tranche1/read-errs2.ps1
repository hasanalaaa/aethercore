$lines = Get-Content -LiteralPath 'C:\AetherCore-P36\incoming\tranche1-rawwire-build.log'
$idx = @()
for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -match '^error') { $idx += $i } }
foreach ($i in $idx) {
    Write-Output ('--- ' + $lines[$i])
    for ($j = 1; $j -le 3 -and ($i + $j) -lt $lines.Count; $j++) { Write-Output $lines[$i + $j] }
}
