[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Package,
    [Parameter(Mandatory=$true)][string]$TestName,
    [switch]$Ignored,
    [switch]$NoCapture
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
$listArgs=@('test','--color','never','--locked','-p',$Package,$TestName,'--','--list')
$listed=@(& cargo @listArgs 2>&1)
if($LASTEXITCODE -ne 0){throw "Unable to enumerate Rust test $Package::$TestName. $($listed -join [Environment]::NewLine)"}
$pattern='(^|::)'+[regex]::Escape($TestName)+': test$'
$matches=@($listed|Where-Object{"$_" -match $pattern})
if($matches.Count -ne 1){throw "Rust test selector must resolve to exactly one test: $Package::$TestName (matched $($matches.Count))."}
$runArgs=@('test','--color','never','--locked','-p',$Package,$TestName,'--')
if($Ignored){$runArgs+='--ignored'}
if($NoCapture){$runArgs+='--nocapture'}
& cargo @runArgs
if($LASTEXITCODE -ne 0){throw "Rust test failed: $Package::$TestName"}
