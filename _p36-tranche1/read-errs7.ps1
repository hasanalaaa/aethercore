$lines = [System.IO.File]::ReadAllText('C:\AetherCore-P36\incoming\tranche1-rawwire-build2.log') -split "`r?`n"
$hits = New-Object System.Collections.Generic.List[string]
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -like 'error*') {
        $hits.Add($lines[$i])
        if ($i + 1 -lt $lines.Count) { $hits.Add($lines[$i + 1]) }
    }
}
[System.IO.File]::WriteAllLines('C:\Users\Public\errs-out.txt', $hits)
Write-Output ("wrote " + $hits.Count)
