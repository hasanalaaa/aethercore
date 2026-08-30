Write-Output '=== SYSTEM token groups ==='
(& whoami.exe /groups 2>&1 | Out-String) -split "`r?`n" | Where-Object { $_ -match 'S-1-1-0|S-1-5-11|S-1-5-6|S-1-5-32-545|S-1-5-18|Mandatory' } | ForEach-Object { Write-Output $_.Trim() }
Write-Output '=== pipe DACL check via pipe SD (from service side, use GetSecurityInfo via powershell? use sc-style check: enumerate with PipeAccess via .NET is not available) ==='
Write-Output '=== whoami for reference ==='
whoami.exe
