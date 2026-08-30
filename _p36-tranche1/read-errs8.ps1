$log = [System.IO.File]::ReadAllText('C:\AetherCore-P36\incoming\tranche1-rawwire-build2.log')
$log = $log.Replace([char]0, '')
$lines = $log -split "`r?`n"
Write-Output ("lines: " + $lines.Count)
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i].Contains('error')) {
        Write-Output ('[' + $i + '] ' + $lines[$i])
    }
}
