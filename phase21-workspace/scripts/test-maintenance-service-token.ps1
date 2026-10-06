#Requires -Version 7.0
# Runs the token verifier's own native probe against this process: no service, no elevation.
# Every SID it reads must decode as a real SID string; Everyone (S-1-1-0) is in any user token.
$ErrorActionPreference='Stop'
$source=Get-Content (Join-Path $PSScriptRoot 'verify-maintenance-service-token.ps1') -Raw
$native=[regex]::Match($source,"(?s)\`$native = @'\r?\n(.*?)\r?\n'@").Groups[1].Value
if (-not $native) { throw 'Native token probe not found in the verifier.' }
Add-Type -TypeDefinition $native -Language CSharp
$token=[AetherCoreServiceTokenProbe]::Inspect([uint32]$PID)
$sids=@($token.Groups | ForEach-Object { $_.Sid })
if ($sids.Count -eq 0) { throw 'The probe read no token groups.' }
$bad=@($sids | Where-Object { $_ -notmatch '^S-1-\d+(-\d+)+$' })
if ($bad.Count) { throw "The probe decoded $($bad.Count) of $($sids.Count) SIDs as non-SID text; ConvertSidToStringSid is bound to the wrong character set." }
if ($sids -notcontains 'S-1-1-0') { throw 'Everyone (S-1-1-0) was not read from this process token.' }
# The evidence path: relative to the workspace, or used as given when absolute (verify-installer-security).
$ast=[Management.Automation.Language.Parser]::ParseInput($source,[ref]$null,[ref]$null)
$assign=$ast.Find({param($n) $n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -eq '$out'},$true)
if (-not $assign) { throw 'The verifier no longer resolves its evidence path in $out.' }
$resolve=[scriptblock]::Create("param(`$Root,`$OutputPath) $($assign.Extent.Text); `$out")
$Root='C:\fixture-root'
if ((& $resolve $Root 'C:\elsewhere\token.json') -ne 'C:\elsewhere\token.json') { throw 'An absolute evidence path was nested under the workspace root.' }
if ((& $resolve $Root 'out\token.json') -ne (Join-Path $Root 'out\token.json')) { throw 'A relative evidence path no longer resolves under the workspace root.' }
Write-Output "TOKEN_PROBE_PASS: $($sids.Count) group SIDs decoded, Everyone present; absolute and relative evidence paths resolve."
