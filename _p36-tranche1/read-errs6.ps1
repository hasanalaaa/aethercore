$lines = Get-Content -LiteralPath 'C:\AetherCore-P36\incoming\tranche1-rawwire-build.log' -Encoding Unicode
Write-Output ("total lines: " + $lines.Count)
$hits = New-Object System.Collections.Generic.List[string]
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i].StartsWith('error')) {
        $hits.Add($lines[$i])
        if ($i + 1 -lt $lines.Count) { $hits.Add($lines[$i + 1]) }
    }
}
[System.IO.File]::WriteAllLines('C:\Users\Public\errs-out.txt', $hits)
Write-Output ("written " + $hits.Count + " lines")
