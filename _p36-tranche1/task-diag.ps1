$taskName = 'P36T1ProbeStandardUser'
Write-Output '=== task query ==='
schtasks /Query /TN $taskName /V /FO LIST 2>&1 | Select-String -Pattern 'TaskName|Status|Last Run Time|Last Result|Logon Mode|Run As User' | ForEach-Object { Write-Output $_.Line }
Write-Output '=== task xml (password not included) ==='
schtasks /Query /TN $taskName /XML 2>&1 | Out-String | Write-Output
Write-Output '=== P36StandardUser account state ==='
net user P36StandardUser | Select-String -Pattern 'Account active|Password last set|Logon script|Account expires|Global Group|Local Group' | ForEach-Object { Write-Output $_ }
