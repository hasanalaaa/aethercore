#Requires -Version 7.0
# Actual producer functions with isolated controllers and native cmd.exe fixture only.
# No installed app, product helper, scheduled task or service is invoked.
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$tokens=$null;$errors=$null
$source=Join-Path $PSScriptRoot 'p87-installed-acceptance.ps1'
$ast=[System.Management.Automation.Language.Parser]::ParseFile($source,[ref]$tokens,[ref]$errors)
if ($errors.Count) { throw 'Actual producer did not parse.' }
Write-Host ('PRODUCER_SHA256=' + (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant())
Write-Host ('TEST_SHA256=' + (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant())
$idAssignment=$ast.FindAll({param($a) $a -is [System.Management.Automation.Language.AssignmentStatementAst] -and $a.Left.Extent.Text -eq '$ids'},$true)[0]
Invoke-Expression $idAssignment.Extent.Text
foreach($name in @('Assert-OwnedText','Invoke-Installed','Case','Wait-Terminal','Wait-RenderedScan','Invoke-CareSmoke')) {
  $fn=@($ast.EndBlock.Statements | Where-Object { $_ -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $_.Name -eq $name })[0]
  Invoke-Expression $fn.Extent.Text
}
$jsDefinition=@($ast.EndBlock.Statements | Where-Object { $_ -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $_.Name -eq 'Js' })[0]
Invoke-Expression ($jsDefinition.Extent.Text.Replace('function Js(', 'function Invoke-ActualJs('))
function Assert([bool]$condition,[string]$message){if(-not $condition){throw $message};Write-Host "PASS: $message"}
function Reject([scriptblock]$test,[string]$message){$rejected=$false;try{& $test}catch{$rejected=$true};Assert $rejected $message}
$renderedScanId='';$echo=$true
$renderExpressions=[Collections.Generic.List[string]]::new()
$replies=@{};$calls=[Collections.Generic.List[string]]::new()
function Js([string]$Expression){
  if($echo){return $Expression}
  if($Expression -match 'dataset.scanId'){
    $renderExpressions.Add($Expression)
    if($Expression -notmatch 'dataset\.scanId===("(?:\\.|[^"\\])*")'){throw 'Rendered query omitted its exact JSON identity comparison.'}
    return $script:renderedScanId -eq ($Matches[1]|ConvertFrom-Json)
  }
  [void]$calls.Add($Expression)
  if($Expression -notmatch "invoke\('([^']+)',") { throw 'Unexpected expression' }
  $command=$Matches[1]
  if(-not $replies.ContainsKey($command)){throw "Unexpected command: $command"}
  $queue=$replies[$command]
  if($queue.Count -gt 1){return $queue.Dequeue()}
  return $queue.Peek()
}
function Queue([string]$command,[object[]]$values){$q=[Collections.Generic.Queue[object]]::new();foreach($v in $values){$q.Enqueue($v)};$replies[$command]=$q}
function Start-Sleep { param($Milliseconds) [Threading.Thread]::Sleep(1) }
Assert ((Invoke-Installed start_perf_sampling @{intervalMs=1000}) -match '"intervalMs":1000') 'actual InvokeArgs preserves intervalMs'
Assert ((Invoke-Installed cancel_repair_assessment @{assessmentId='owned-id'}) -match '"assessmentId":"owned-id"') 'actual InvokeArgs preserves assessmentId'
Assert-OwnedText @('Windows temporary files','Archived Windows Error Reporting files. Keep these when diagnosing crashes.') en
Write-Host 'PASS: approved cleanup English files prose'
Assert-OwnedText @('ملاحظات القرص NVMe المقاسة') ar
Reject {Assert-OwnedText @('Files · 3') en} 'isolated Files token rejected'
Reject {Assert-OwnedText @('ملاحظات القرص Measured disk observations') ar} 'mixed AR/EN prose rejected'
Reject {Assert-OwnedText @('processTop · NotCollected') en} 'raw provider identifiers rejected'
$echo=$false;$deadline=[DateTime]::UtcNow.AddSeconds(5)
Queue get_diagnostics_snapshot @(@{state='Collecting';scanId='owned'},@{state='Partial';scanId='owned'})
$terminal=Wait-Terminal get_diagnostics_snapshot owned scanId
Assert ($terminal.state -eq 'Partial') 'actual wait does not return Collecting'
Queue get_diagnostics_snapshot @(@{state='Ready';scanId='other'})
Reject {Wait-Terminal get_diagnostics_snapshot owned scanId} 'different scan identity rejected'
Queue get_diagnostics_snapshot @(@{state='Bogus';scanId='owned'})
Reject {Wait-Terminal get_diagnostics_snapshot owned scanId} 'unknown state rejects terminal qualification'
$workers=@{samplingStarted=$false;assessmentStarted=$false;assessmentId='';careStarted=$false;helper=$null}
$doc=[ordered]@{cases=@(@{disposition='blocked';reason='Not executed.'})}
Case 0 {$workers.samplingStarted=$true;$workers.assessmentStarted=$true;$workers.assessmentId='owned-id';throw 'Planted case failure'}
Assert ($workers.samplingStarted -and $workers.assessmentStarted -and $workers.assessmentId -eq 'owned-id') 'shared workers survive child scope and exception'
$outerTry=@($ast.EndBlock.Statements | Where-Object {$_ -is [System.Management.Automation.Language.TryStatementAst]})[-1]
$body=$outerTry.Finally.Extent.Text
$finally=[scriptblock]::Create($body.Substring(1,$body.Length-2))
function Save-Json($Path,$Value) { }
function Fake-Socket { $s=[pscustomobject]@{disposed=$false};$s|Add-Member ScriptMethod Dispose {$this.disposed=$true};return $s }
function Fake-Desktop([bool]$canExit) {
 $d=[pscustomobject]@{HasExited=$false;canExit=$canExit;waitMs=0;closeCalled=$false}
 $d|Add-Member ScriptMethod CloseMainWindow {$this.closeCalled=$true;return $true}
 $d|Add-Member ScriptMethod WaitForExit {param($ms)$this.waitMs=$ms;return $this.canExit}
 return $d
}
function Reset-Finally {
 $script:doc=[ordered]@{cases=@(@{disposition='passed';reason='Synthetic actual-function proof'})}
 $script:workers=@{samplingStarted=$false;assessmentStarted=$false;assessmentId='';careStarted=$false;helper=$null}
 $script:socket=Fake-Socket;$script:desktop=Fake-Desktop $true
 $script:fixture=$null;$script:ReadOnlyInstalled=$false;$script:OutputPath='C:\dev\lanes\l1\unused-synthetic.json'
 $script:replies=@{};$script:calls=[Collections.Generic.List[string]]::new()
 Queue get_repair_assessment @(@{state='Ready';assessmentId='owned-id'})
 Queue get_care_status @(@{state='Completed'})
 Queue stop_perf_sampling @(@{})
 Queue get_diagnostics_snapshot @(@{state='Ready';scanId='owned-scan'})
 Queue get_cleanup_snapshot @(@{state='Ready';scanId='owned-cleanup'})
}
Reset-Finally
$workers.samplingStarted=$true;$workers.assessmentStarted=$true;$workers.assessmentId='owned-id';$workers.careStarted=$true
Queue get_repair_assessment @(@{state='Scanning';assessmentId='owned-id'},@{state='Cancelled';assessmentId='owned-id'})
Queue cancel_repair_assessment @(@{state='Cancelled';assessmentId='owned-id'})
Queue get_care_status @(@{state='Running'},@{state='Completed'})
& $finally
Assert ($doc.worker_ownership_released -and $doc.ok -and $socket.disposed) 'actual finally drains known workers and disposes fake socket'
Assert ($desktop.closeCalled -and $desktop.waitMs -eq 10000) 'actual finally requests bounded desktop exit'
Assert (@($calls | Where-Object {$_ -match "invoke\('cancel_repair_assessment'.*owned-id"}).Count -eq 1) 'only owned assessment cancelled by actual finally'
Reset-Finally
$workers.assessmentStarted=$true;$workers.assessmentId='owned-id'
Queue get_repair_assessment @(@{state='Scanning';assessmentId='other-id'})
& $finally
Assert (-not $doc.worker_ownership_released -and -not $doc.ok) 'wrong live assessment keeps lifecycle blocked'
Assert (@($calls|Where-Object {$_ -match "invoke\('cancel_repair_assessment'"}).Count -eq 0) 'other assessment is not cancelled'
Reset-Finally
$desktop=Fake-Desktop $false
& $finally
Assert (-not $doc.worker_ownership_released -and -not $doc.desktop_closed -and -not $doc.ok) 'desktop exit failure blocks lifecycle'
Reset-Finally
$socket=$null;$workers.careStarted=$true
& $finally
Assert (-not $doc.worker_ownership_released -and -not $doc.ok) 'lost transport with owned worker blocks lifecycle'
Write-Host 'PASS: actual producer ownership controls'

Reset-Finally
Queue get_diagnostics_snapshot @(@{state='Collecting';scanId='owned-scan'},@{state='Ready';scanId='owned-scan'})
Queue get_cleanup_snapshot @(@{state='Scanning';scanId='owned-cleanup'},@{state='Ready';scanId='owned-cleanup'})
& $finally
Assert ($doc.worker_ownership_released -and $doc.ok) 'finally waits diagnostics and cleanup to known terminal'
Assert (@($calls|Where-Object {$_ -match "invoke\('get_(diagnostics|cleanup)_snapshot'"}).Count -ge 4) 'finally checks both read-only scan workers'
Reset-Finally
Queue get_cleanup_snapshot @(@{state='Bogus';scanId='owned-cleanup'})
& $finally
Assert (-not $doc.worker_ownership_released -and -not $doc.ok) 'unknown cleanup state blocks final release'


$renderedScanId='owned-scan';$deadline=[DateTime]::UtcNow.AddSeconds(2)
Wait-RenderedScan '.storage-grid' 'owned-scan'
Write-Host 'PASS: actual rendered scan identity admitted'
$renderedScanId='different-scan';$deadline=[DateTime]::UtcNow.AddSeconds(-1)
Reject {Wait-RenderedScan '.storage-grid' 'owned-scan'} 'stale rendered scan fails at deadline'
Reject {Wait-RenderedScan '.cleanup-list' ''} 'empty scan identity cannot qualify UI'
Assert ($renderExpressions[0].Contains("document.querySelector('.storage-grid')") -and $renderExpressions[0].Contains('requestAnimationFrame(()=>requestAnimationFrame(r))')) 'actual rendered wait observes requested DOM after paint'
$cdpReply=@{result=@{value=$true}};$cdpArguments=$null
function Cdp($Method,$Parameters){$script:cdpArguments=@{Method=$Method;Parameters=$Parameters};return $script:cdpReply}
Assert ((Invoke-ActualJs 'fixture-expression') -eq $true -and $cdpArguments.Method -eq 'Runtime.evaluate' -and $cdpArguments.Parameters.expression -eq 'fixture-expression' -and $cdpArguments.Parameters.awaitPromise -eq $true -and $cdpArguments.Parameters.returnByValue -eq $true) 'actual Js forwards awaited expression and typed value through isolated CDP'
$cdpReply=@{result=@{value=$false}}
Assert ((Invoke-ActualJs 'fixture-expression') -eq $false) 'actual Js preserves false rendered result'
$cdpReply=@{result=@{}}
Assert ($null -eq (Invoke-ActualJs 'fixture-expression')) 'actual Js preserves absent rendered result'
$cdpReply=@{exceptionDetails=@{text='synthetic renderer failure'}}
Reject {Invoke-ActualJs 'fixture-expression'} 'actual Js rejects renderer exception'

$gaps=[Collections.Generic.List[string]]::new()
$receiptIf=@($ast.EndBlock.Statements | Where-Object { $_ -is [System.Management.Automation.Language.IfStatementAst] -and $_.Clauses[0].Item1.Extent.Text -eq '$VerifyCareRunId' })[0]
$receiptCheck=[scriptblock]::Create($receiptIf.Extent.Text)
$receipt=@{schema='fixture-schema';source_commit=('a'*40);bundle_sha256=('b'*64);locale='en';ordinary_user=$true;read_only=$false;token=@{sid='S-1-5-21-fixture';elevated=$false};care_run_id='11111111-2222-4333-8444-555555555555';worker_ownership_released=$true;desktop_closed=$true;restart_pending=$true;cases=@(@{id='p76-hardware-owned-text';witness='prior-case'},@{id='p76-performance-provider-labels'},@{id='p76-cleanup-owned-text'},@{id='p76-care-timeline-persistence';checks=@{reconnect=$true;restart=$false};witnesses=@();disposition='blocked';reason='restart pending'},@{id='p76-repair-assessment-terminal'},@{id='p76-care-eligibility-explanation'})}
function Invoke-ReceiptFixture([hashtable]$Receipt) {
    $ExpectedSourceSha='a'*40;$ExpectedBundleSha256='b'*64;$Locale='en'
    $VerifyCareRunId='11111111-2222-4333-8444-555555555555';$ReadOnlyInstalled=$false
    $identity=@{User=@{Value='S-1-5-21-fixture'}};$doc=@{schema='fixture-schema'};$OutputPath='fixture-only'
    function Get-Content { param($LiteralPath,[switch]$Raw) return ($Receipt | ConvertTo-Json -Depth 12) }
    . $receiptCheck
    return $doc
}
$preserved=Invoke-ReceiptFixture $receipt
Assert ($preserved.cases[0].witness -eq 'prior-case' -and $preserved.care_run_id -eq $receipt.care_run_id) 'actual verification preserves prior receipt and run identity'
foreach($field in @('schema','source_commit','bundle_sha256','locale','care_run_id','worker_ownership_released','desktop_closed')) {
    $wrong=$receipt.Clone();$wrong[$field]=if($field -in @('worker_ownership_released','desktop_closed')){$false}else{'wrong-pin'}
    Reject { Invoke-ReceiptFixture $wrong } "actual verification rejects mismatched $field"
}
$wrong=$receipt.Clone();$wrong.token=@{sid='another-owner'}
Reject { Invoke-ReceiptFixture $wrong } 'actual verification rejects another owner receipt'
foreach($field in @('worker_ownership_released','desktop_closed')) {
    $wrong=$receipt.Clone();$wrong[$field]='false'
    $rejected=$false;try{[void](Invoke-ReceiptFixture $wrong)}catch{$rejected=$true}
    if($rejected){Write-Host "PASS: receipt rejects string false $field"}else{$gaps.Add("Receipt accepts string false $field");Write-Host "FINDING: receipt accepts string false $field"}
}
foreach($field in @('worker_ownership_released','desktop_closed','ordinary_user','read_only','restart_pending')) {
    foreach($value in @('false',1,$null)) {
        $wrong=$receipt.Clone();$wrong[$field]=$value
        $rejected=$false;try{[void](Invoke-ReceiptFixture $wrong)}catch{$rejected=$true}
        if(-not $rejected){$gaps.Add("Receipt accepts untyped $field ($value)")}
    }
    $wrong=$receipt.Clone();$wrong.Remove($field)
    $rejected=$false;try{[void](Invoke-ReceiptFixture $wrong)}catch{$rejected=$true}
    if(-not $rejected){$gaps.Add("Receipt accepts missing $field")}
}
foreach($field in @('ordinary_user','read_only','restart_pending','elevated','reconnect','restart')) {
    $wrong=($receipt|ConvertTo-Json -Depth 12|ConvertFrom-Json -AsHashtable)
    switch($field){
        'elevated' {$wrong.token.elevated=$true}
        'reconnect' {$wrong.cases[3].checks.reconnect=$false}
        'restart' {$wrong.cases[3].checks.restart=$true}
        'read_only' {$wrong.read_only=$true}
        default {$wrong[$field]=$false}
    }
    $rejected=$false;try{[void](Invoke-ReceiptFixture $wrong)}catch{$rejected=$true}
    if($rejected){Write-Host "PASS: receipt rejects ineligible $field"}else{$gaps.Add("Receipt accepts ineligible $field")}
}
foreach($field in @('elevated','reconnect','restart')) {
    $wrong=($receipt|ConvertTo-Json -Depth 12|ConvertFrom-Json -AsHashtable)
    if($field -eq 'elevated'){$wrong.token[$field]='false'}else{$wrong.cases[3].checks[$field]='false'}
    $rejected=$false;try{[void](Invoke-ReceiptFixture $wrong)}catch{$rejected=$true}
    if(-not $rejected){$gaps.Add("Receipt accepts untyped $field")}
    $wrong=($receipt|ConvertTo-Json -Depth 12|ConvertFrom-Json -AsHashtable)
    if($field -eq 'elevated'){$wrong.token.Remove($field)}else{$wrong.cases[3].checks.Remove($field)}
    $rejected=$false;try{[void](Invoke-ReceiptFixture $wrong)}catch{$rejected=$true}
    if(-not $rejected){$gaps.Add("Receipt accepts missing $field")}
}
$wrong=($receipt|ConvertTo-Json -Depth 12|ConvertFrom-Json -AsHashtable);$wrong['blocked_reason']='earlier producer fault'
Reject { Invoke-ReceiptFixture $wrong } 'global blocked receipt cannot qualify restart'
$wrong=($receipt|ConvertTo-Json -Depth 12|ConvertFrom-Json -AsHashtable);$wrong.cases+=@($wrong.cases[3].Clone())
Reject {Invoke-ReceiptFixture $wrong} 'duplicate Care identity cannot qualify restart'
$reordered=($receipt|ConvertTo-Json -Depth 12|ConvertFrom-Json -AsHashtable);$swap=$reordered.cases[0];$reordered.cases[0]=$reordered.cases[3];$reordered.cases[3]=$swap
$publicationIf=$ast.FindAll({param($a) $a -is [System.Management.Automation.Language.IfStatementAst] -and $a.Clauses[0].Item1.Extent.Text -eq '$VerifyCareRunId' -and $a.Extent.Text.Contains('care-after-service-restart.txt')},$true)[0]
$publicationBody=$publicationIf.Clauses[0].Item2.Extent.Text
$publication=[scriptblock]::Create($publicationBody.Substring(1,$publicationBody.Length-2))
function Invoke-RestartPublicationFixture([hashtable]$Receipt) {
    $ExpectedSourceSha='a'*40;$ExpectedBundleSha256='b'*64;$Locale='en'
    $VerifyCareRunId='11111111-2222-4333-8444-555555555555';$ReadOnlyInstalled=$false
    $identity=@{User=@{Value='S-1-5-21-fixture'}};$doc=@{schema='fixture-schema'};$OutputPath='fixture-only';$capture=$fixtureRoot
    $pageCalls=[Collections.Generic.List[string]]::new()
    function Get-Content {param($LiteralPath,[switch]$Raw) return ($Receipt|ConvertTo-Json -Depth 12)}
    function Invoke-CareSmoke($Log,[string[]]$ProcessArgs) {
        Assert ($ProcessArgs.Count -eq 2 -and $ProcessArgs[0] -eq '--verify-run-id' -and $ProcessArgs[1] -eq $VerifyCareRunId) 'actual verification requests only the pinned native run'
        [IO.File]::WriteAllText($Log,"SMOKE: persisted-care-run=$VerifyCareRunId")
    }
    function Page($Name){$pageCalls.Add($Name)}
    function Witness($Path){return @{path=$Path}}
    function Capture($Name,$Selector){return @(@{path='isolated-capture'})}
    . $receiptCheck
    . $publication
    Assert ($pageCalls.Count -eq 1 -and $pageCalls[0] -eq 'activity') 'actual verification visits Activity via isolated page controller'
    return $doc
}

$fixtureRoot=Join-Path ([IO.Path]::GetTempPath()) ('aethercore-p87-producer-' + [Guid]::NewGuid().ToString('N'))
[void](New-Item -ItemType Directory (Join-Path $fixtureRoot 'acceptance'))
Copy-Item -LiteralPath (Join-Path $env:SystemRoot 'System32/cmd.exe') -Destination (Join-Path $fixtureRoot 'acceptance/care_smoke.exe')
$ReleaseRoot=$fixtureRoot
try {
    $published=Invoke-RestartPublicationFixture $reordered
    Assert ($published.cases[0].id -eq 'p76-care-timeline-persistence' -and $published.cases[0].checks.restart -eq $true -and $published.cases[0].disposition -eq 'passed' -and $published.cases[0].witnesses.Count -eq 2 -and $published.cases[3].witness -eq 'prior-case' -and -not $published.restart_pending) 'actual restart publication updates the selected Care identity and preserves reordered other cases'
    # Change only the ambient root: all native creation/hash/timestamp statements stay actual.
    $fixtureCreationRoot=Join-Path $fixtureRoot 'owned-creation';[void](New-Item -ItemType Directory $fixtureCreationRoot)
    $creationIf=$ast.FindAll({param($a) $a -is [System.Management.Automation.Language.IfStatementAst] -and $a.Clauses[0].Item1.Extent.Text -eq '-not $VerifyCareRunId'},$true)[0]
    $creationText=$creationIf.Extent.Text.Replace("`$tempRoot=Join-Path `$env:SystemRoot 'Temp'",'$tempRoot=$fixtureCreationRoot')
    $creationCheck=[scriptblock]::Create($creationText)
    function Invoke-CreationFixture([string]$Verification='') {
        $VerifyCareRunId=$Verification;$fixture=$null;$doc=@{}
        . $creationCheck
        return @{Path=$fixture;Receipt=$doc}
    }
    $created=Invoke-CreationFixture
    Assert ($created.Path.StartsWith($fixtureCreationRoot + [IO.Path]::DirectorySeparatorChar) -and $created.Receipt.fixture.disposable -eq $true) 'actual fixture creation owns a unique confined disposable file'
    Assert ((Get-FileHash -LiteralPath $created.Path -Algorithm SHA256).Hash.ToLowerInvariant() -eq $created.Receipt.fixture.sha256 -and (Get-Item -LiteralPath $created.Path).LastWriteTimeUtc -lt [DateTime]::UtcNow.AddDays(-2)) 'actual fixture records hash and old eligibility timestamp'
    Assert ($null -eq (Invoke-CreationFixture '11111111-2222-4333-8444-555555555555').Path) 'restart verification creates no cleanup fixture'
    $cleanupIf=$ast.FindAll({param($a) $a -is [System.Management.Automation.Language.IfStatementAst] -and $a.Clauses[0].Item1.Extent.Text.StartsWith('$fixture -and $doc.worker_ownership_released')},$true)[0]
    $cleanupCheck=[scriptblock]::Create($cleanupIf.Extent.Text)
    function Invoke-CleanupFixture([bool]$Released,[hashtable]$Receipt=$created.Receipt) {
        $fixture=$created.Path;$doc=$Receipt.Clone();$doc['worker_ownership_released']=$Released
        $removed=[Collections.Generic.List[string]]::new()
        function Remove-Item { param($LiteralPath) $removed.Add($LiteralPath) }
        . $cleanupCheck
        return @{Removed=$removed.ToArray();Receipt=$doc}
    }
    Assert ((Invoke-CleanupFixture $false).Removed.Count -eq 0) 'actual cleanup preserves fixture while worker ownership is live'
    $unchanged=Invoke-CleanupFixture $true
    Assert ($unchanged.Removed.Count -eq 1 -and $unchanged.Removed[0] -eq $created.Path) 'actual cleanup targets only its unchanged owned file'
    foreach($field in @('path','disposable','bytes','sha256')) {
        $wrong=($created.Receipt|ConvertTo-Json -Depth 12|ConvertFrom-Json -AsHashtable)
        $wrong.fixture[$field]=switch($field){'path'{'unowned-path'};'disposable'{'true'};'bytes'{0};'sha256'{'0'*64}}
        $unowned=Invoke-CleanupFixture $true $wrong
        Assert ($unowned.Removed.Count -eq 0 -and -not $unowned.Receipt.worker_ownership_released) "actual cleanup rejects invalid fixture $field"
    }
    [IO.File]::WriteAllText($created.Path,('X' * $created.Receipt.fixture.bytes))
    $changed=Invoke-CleanupFixture $true
    if($changed.Removed.Count -eq 0 -and -not $changed.Receipt.worker_ownership_released){Write-Host 'PASS: changed fixture cannot be removed'}else{$gaps.Add('Cleanup removes changed fixture by path without ownership proof');Write-Host 'FINDING: cleanup removes changed fixture by path without ownership proof'}
    Reset-Finally
    $deadline=[DateTime]::UtcNow.AddSeconds(10)
    $log=Join-Path $fixtureRoot 'native-success.txt'
    Invoke-CareSmoke $log @('/d','/c','echo native-helper-proof')
    Assert ((Get-Content -LiteralPath $log -Raw) -match 'native-helper-proof') 'actual process argument list/output capture'
    $deadline=[DateTime]::UtcNow.AddSeconds(10)
    Reject {Invoke-CareSmoke (Join-Path $fixtureRoot 'native-failure.txt') @('/d','/c','exit /b 7')} 'actual nonzero native process rejected'
    $workers.helper=$null;$deadline=[DateTime]::UtcNow.AddSeconds(-1)
    Reject {Invoke-CareSmoke (Join-Path $fixtureRoot 'not-started.txt') @('/d','/c','echo should-not-start')} 'expired budget starts no helper'
    Assert ($null -eq $workers.helper) 'expired helper has no process ownership'
    $deadline=[DateTime]::UtcNow.AddMilliseconds(350)
    Reject {Invoke-CareSmoke (Join-Path $fixtureRoot 'native-timeout.txt') @('/d','/c','ping -n 4 127.0.0.1 >nul')} 'native helper timeout is declared'
    Assert (-not $workers.helper.HasExited) 'timed-out native process preserved without forced kill'
    & $finally
    Assert (-not $doc.worker_ownership_released -and -not $doc.ok) 'live timed-out helper blocks lifecycle after other workers drain'
    Assert ($workers.helper.WaitForExit(10000)) 'native timeout fixture exits naturally'
    # Actual Case5 body, with only IPC/DOM controllers isolated. A completed pre-cleanup
    # snapshot is intentionally still Ready and eligible, just as the cleaner retains it.
    $case5Command=$ast.FindAll({param($a) $a -is [System.Management.Automation.Language.CommandAst] -and $a.GetCommandName() -eq 'Case' -and $a.CommandElements[1].Extent.Text -eq '5'},$true)[0]
    $case5Text=$case5Command.CommandElements[2].ScriptBlock.Extent.Text
    $case5Body=[scriptblock]::Create($case5Text.Substring(1,$case5Text.Length-2))
    function Invoke-NoOpPreparationFixture([bool]$ReadOnly=$false,[bool]$ForeignScan=$false) {
        $ReadOnlyInstalled=$ReadOnly;$Locale='en';$deadline=[DateTime]::UtcNow.AddSeconds(5)
        $doc=@{cases=@($ids | ForEach-Object {@{id=$_;disposition='blocked';reason='Not executed.';checks=@{};witnesses=@()}})}
        $events=[Collections.Generic.List[string]]::new()
        $state=@{started=$false;completed=$false;prepared=$false;polls=0}
        function Page($Name){$events.Add("page:$Name")}
        function Capture($Name,$Selector){return @(@{path='isolated-noop-witness'})}
        function Js([string]$Expression) {
            if ($Expression -match "invoke\('([^']+)',") {
                $command=$Matches[1];$events.Add($command)
                switch ($command) {
                    'start_cleanup_scan' {$state.started=$true;return @{state='Scanning';scanId='fresh-owned'}}
                    'get_cleanup_snapshot' {
                        if ($ForeignScan) {return @{state='Scanning';scanId='existing-unowned';candidates=@()}}
                        if (-not $state.started) {return @{state='Ready';scanId='old-pre-cleanup';candidates=@(@{selectedByDefault=$true;requiresExplicitConfirmation=$false})}}
                        $state.polls++
                        if ($state.polls -eq 1) {return @{state='Scanning';scanId='fresh-owned';candidates=@()}}
                        $state.completed=$true;return @{state='Ready';scanId='fresh-owned';candidates=@()}
                    }
                    default {throw "Unexpected actual Case5 IPC command $command"}
                }
            }
            if ($Expression.Contains('.overview-header-side button.secondary')) {
                $events.Add('actual-ui-prepare')
                if (-not $state.completed) {throw 'Actual UI preparation reused the old Ready inventory.'}
                $state.prepared=$true;return $true
            }
            if ($Expression.Contains('.care-panel p')) {
                if (-not $state.prepared) {throw 'No actual no-op preparation was requested.'}
                return @('Nothing eligible for automatic care was found: the default cleanup category had nothing to clean.')
            }
            throw 'Unexpected Case5 DOM controller expression.'
        }
        Case 5 $case5Body
        return @{Case=$doc.cases[5];Events=$events.ToArray();State=$state}
    }
    $noOp=Invoke-NoOpPreparationFixture
    Assert ($noOp.Case.disposition -eq 'passed' -and $noOp.Case.checks.no_op_explained -eq $true) 'actual Case5 prepares no-op only after a fresh owned terminal read'
    Assert ($noOp.State.polls -ge 2 -and [Array]::IndexOf($noOp.Events,'start_cleanup_scan') -lt [Array]::IndexOf($noOp.Events,'actual-ui-prepare')) 'actual Case5 waits through Scanning before clicking UI preparation'
    $foreign=Invoke-NoOpPreparationFixture -ForeignScan $true
    Assert ($foreign.Case.disposition -ne 'passed' -and 'start_cleanup_scan' -notin $foreign.Events -and 'actual-ui-prepare' -notin $foreign.Events) 'actual Case5 preserves an existing cleanup worker without replacement or prepare'
    $readOnly=Invoke-NoOpPreparationFixture -ReadOnly $true
    Assert ('start_cleanup_scan' -notin $readOnly.Events -and 'actual-ui-prepare' -notin $readOnly.Events) 'actual Case5 read-only mode starts no scan or preview'
    if($gaps.Count){throw ($gaps -join '; ')}
    Write-Host 'P87_PRODUCER_FIXTURES_PASS'
} finally {
    if ($workers.helper -and -not $workers.helper.HasExited) {
        [void]$workers.helper.WaitForExit(10000)
    }
    if (-not $workers.helper -or $workers.helper.HasExited) { Remove-Item -LiteralPath $fixtureRoot -Recurse -Force }
    else { Write-Warning "Native fixture still owns $fixtureRoot; left intact without forced kill." }
}
