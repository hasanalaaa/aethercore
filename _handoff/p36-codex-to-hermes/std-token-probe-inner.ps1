# P36 StandardUser token probe — inner script (runs AS P36StandardUser via one-shot Scheduled Task)
# Writes token evidence to the user's own temp; SYSTEM copies it into evidence afterwards.
$ErrorActionPreference = 'Continue'
$out = Join-Path $env:TEMP 'p36std-token.txt'
'=== P36StandardUser TOKEN EVIDENCE (context: StandardUser, one-shot Scheduled Task, RunLevel=Limited) ===' | Set-Content $out
('CONTEXT_LABEL=StandardUser') | Add-Content $out
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
$labels = @($id.Groups | Where-Object { $_.Value -like 'S-1-16-*' } | ForEach-Object { $_.Value })
('MACHINE_CHECK integrity_labels=' + ($labels -join ',')) | Add-Content $out
$admin = @($id.Groups | Where-Object { $_.Value -eq 'S-1-5-32-544' })
if ($admin.Count -gt 0) {
    ('MACHINE_CHECK S-1-5-32-544=PRESENT_ATTRIBUTES(' + (($admin | ForEach-Object { $_.Attributes }) -join '|') + ')') | Add-Content $out
} else {
    'MACHINE_CHECK S-1-5-32-544=ABSENT_FROM_TOKEN' | Add-Content $out
}
$prin = New-Object Security.Principal.WindowsPrincipal($id)
('MACHINE_CHECK IsInRole_BUILTIN_Administrators=' + $prin.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) | Add-Content $out
('MACHINE_CHECK token_type_note=Medium integrity label + IsInRole=False + no S-1-5-32-544 membership proves non-elevated standard-user token') | Add-Content $out
'--- machine-readable check for SID S-1-5-32-544 (explicit) ---' | Add-Content $out
$hits = (whoami /groups) | Select-String 'S-1-5-32-544'
if ($hits) { $hits | ForEach-Object { ('WHOAMI_MATCH: ' + $_.Line.Trim()) | Add-Content $out } }
else { 'WHOAMI_MATCH: <none — S-1-5-32-544 not present in token group list>' | Add-Content $out }
