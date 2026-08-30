# P36 Admin token probe — inner (runs AS P36Admin via one-shot Scheduled Task, RunLevel Highest)
$ErrorActionPreference = 'Continue'
$out = Join-Path $env:TEMP 'p36admin-token.txt'
'=== P36Admin TOKEN EVIDENCE (context: Administrator, one-shot Scheduled Task, RunLevel=Highest) ===' | Set-Content $out
('CONTEXT_LABEL=Administrator') | Add-Content $out
('CAPTURED_UTC=' + (Get-Date).ToUniversalTime().ToString('o')) | Add-Content $out
'--- whoami ---' | Add-Content $out
whoami 2>&1 | Add-Content $out
'--- whoami /user ---' | Add-Content $out
whoami /user 2>&1 | Add-Content $out
'--- whoami /groups ---' | Add-Content $out
whoami /groups 2>&1 | Add-Content $out
'--- machine-readable checks ---' | Add-Content $out
$id = [Security.Principal.WindowsIdentity]::GetCurrent()
('MACHINE_CHECK current_user=' + $id.Name) | Add-Content $out
('MACHINE_CHECK user_sid=' + $id.User.Value) | Add-Content $out
$admin = @($id.Groups | Where-Object { $_.Value -eq 'S-1-5-32-544' })
if ($admin.Count -gt 0) {
    ('MACHINE_CHECK S-1-5-32-544=PRESENT_ATTRIBUTES(' + (($admin | ForEach-Object { $_.Attributes }) -join '|') + ')') | Add-Content $out
} else {
    'MACHINE_CHECK S-1-5-32-544=ABSENT_FROM_TOKEN' | Add-Content $out
}
$prin = New-Object Security.Principal.WindowsPrincipal($id)
('MACHINE_CHECK IsInRole_BUILTIN_Administrators=' + $prin.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) | Add-Content $out
$hits = (whoami /groups) | Select-String 'S-1-5-32-544|S-1-16-12288'
$hits | ForEach-Object { ('WHOAMI_MATCH: ' + $_.Line.Trim()) | Add-Content $out }
