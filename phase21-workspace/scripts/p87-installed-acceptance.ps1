#Requires -Version 7.0
<#
Installed Windows 11 evidence, using the real WebView2/Tauri backend. The lifecycle
harness supplies an ordinary interactive token and an acknowledged disposable VM.
ReadOnlyInstalled captures the owner's existing UI without running Care or servicing;
it always remains ineligible. No frontend fixtures or missing cases become passes.
#>
[CmdletBinding()]
param(
    [string]$ReleaseRoot,
    [string]$ExpectedSourceSha,
    [string]$ExpectedBundleSha256,
    [string]$OutputPath,
    [ValidateSet('en','ar')][string]$Locale = 'en',
    [ValidateRange(30,600)][int]$TimeoutSeconds = 300,
    [switch]$AcknowledgeDisposableMachine,
    # Owner decision D33: the owner's own PC; the lifecycle parent checks the recorded decision.
    [switch]$OwnerHostAccepted,
    [switch]$ReadOnlyInstalled,
    [string]$VerifyCareRunId,
    [switch]$SelfTest
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Assert-OwnedText([string[]]$Text, [string]$Language) {
    if (-not $Text.Count -or @($Text | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }).Count -eq 0) { throw 'Owned text was not exercised.' }
    foreach ($line in $Text) {
        if ($line -match '\b(?:WindowsTemp|UserTemp|WER|processTop|power\.temperature|NotCollected|Degraded)\b' -or $line -cmatch '(?:^|[·•])\s*Files\s*(?:$|[·•])') { throw "Raw identifier in owned text: $line" }
        # Product and API names the Arabic catalog keeps whole (Direct3D: owner PC cleanup, D33 run 6).
        $prose = $line -replace '\b(?:Windows Error Reporting|Direct3D|NVMe|SMART|WHEA|SFC|DISM|CHKDSK|Windows|NTFS)\b',''
        if ($Language -eq 'ar' -and $prose -match '[A-Za-z]{3,}') { throw "English owned text in Arabic: $line" }
    }
}
function Invoke-Installed([string]$Command, [hashtable]$InvokeArgs=@{}) {
    Js "window.__TAURI_INTERNALS__.invoke('$Command', $($InvokeArgs | ConvertTo-Json -Compress))"
}
if ($SelfTest) {
    function Js([string]$Expression) { return $Expression }
    $invocation=Invoke-Installed start_perf_sampling @{intervalMs=1000}
    if ($invocation -notmatch '"intervalMs":1000' -or $invocation -notmatch "invoke\('start_perf_sampling'") { throw 'Actual native invocation lost its named arguments.' }
    Assert-OwnedText @('Measured disk observations') en
    Assert-OwnedText @('Windows temporary files') en
    Assert-OwnedText @('ملاحظات القرص المقاسة') ar
    foreach ($control in @(@('processTop · NotCollected','en'), @('WindowsTemp','ar'), @('Measured disk observations','ar'), @('ملاحظات القرص Measured disk observations','ar'))) {
        $rejected = $false
        try { Assert-OwnedText @($control[0]) $control[1] } catch { $rejected = $true }
        if (-not $rejected) { throw 'Planted owned-text negative control escaped.' }
    }
    $rejected = $false
    try { Assert-OwnedText @() en } catch { $rejected = $true }
    if (-not $rejected) { throw 'An unexercised text case became a pass.' }
    Write-Host 'P87 owned-text controls: PASS'
    exit 0
}
if (-not $IsWindows) { throw 'Native Windows acceptance requires Windows.' }
if (-not $OutputPath) { throw '-OutputPath is required.' }
if ($OwnerHostAccepted -and $AcknowledgeDisposableMachine) { throw 'Choose one host: -OwnerHostAccepted (D33) or -AcknowledgeDisposableMachine.' }
if (-not $ReadOnlyInstalled -and -not $AcknowledgeDisposableMachine -and -not $OwnerHostAccepted) { throw 'Installed mutating acceptance requires an acknowledged disposable machine or the owner host under D33.' }
if (-not $ReadOnlyInstalled -and ($ExpectedSourceSha -notmatch '^[0-9a-f]{40,64}$' -or $ExpectedBundleSha256 -notmatch '^[0-9a-f]{64}$')) { throw 'Exact source and bundle hashes are required.' }

$ids = @('p76-hardware-owned-text','p76-performance-provider-labels','p76-cleanup-owned-text',
    'p76-care-timeline-persistence','p76-repair-assessment-terminal','p76-care-eligibility-explanation')
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$elevated = ([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$os = Get-CimInstance Win32_OperatingSystem
$evidence = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))
$capture = Join-Path $evidence $Locale
New-Item -ItemType Directory -Force -Path $capture | Out-Null
$doc = [ordered]@{ schema='aethercore.p87-installed-acceptance.v1'; source_commit=$ExpectedSourceSha;
    bundle_sha256=$ExpectedBundleSha256; windows_build=[int]$os.BuildNumber;
    windows_product_name=$os.Caption; product_type=$(if ($os.ProductType -eq 1) { 'workstation' } else { 'server' });
    locale=$Locale; host=$(if ($ReadOnlyInstalled) { 'read-only' } elseif ($OwnerHostAccepted) { 'owner-host-d33' } else { 'disposable-vm' });
    ordinary_user=(-not $elevated); token=@{sid=$identity.User.Value;elevated=$elevated};
    observed_utc=[DateTime]::UtcNow.ToString('o'); read_only=[bool]$ReadOnlyInstalled; surface_witnesses=@{};
    cases=@($ids | ForEach-Object { @{id=$_;disposition='blocked';reason='Not executed.';checks=@{};witnesses=@()} }) }
if ($VerifyCareRunId) {
    if ($ReadOnlyInstalled -or $VerifyCareRunId -cnotmatch '^care-[0-9]{1,19}$') { throw 'Restart verification requires the recorded native Care run identity.' }
    $previous=Get-Content -LiteralPath $OutputPath -Raw | ConvertFrom-Json -AsHashtable
    if ($previous.schema -ne $doc.schema -or $previous.host -ne $doc.host -or $previous.source_commit -ne $ExpectedSourceSha -or $previous.bundle_sha256 -ne $ExpectedBundleSha256 -or $previous.locale -ne $Locale -or $previous.token.sid -ne $identity.User.Value -or $previous.care_run_id -ne $VerifyCareRunId -or $previous.worker_ownership_released -isnot [bool] -or -not $previous.worker_ownership_released -or $previous.desktop_closed -isnot [bool] -or -not $previous.desktop_closed) { throw 'Restart verification report does not belong to these bytes and this ordinary user.' }
    $care=@($previous.cases | Where-Object id -eq 'p76-care-timeline-persistence')
    if ($previous.ContainsKey('blocked_reason') -or $previous.read_only -isnot [bool] -or $previous.read_only -or $previous.ordinary_user -isnot [bool] -or -not $previous.ordinary_user -or $previous.token.elevated -isnot [bool] -or $previous.token.elevated -or $previous.restart_pending -isnot [bool] -or -not $previous.restart_pending -or $care.Count -ne 1 -or $care[0].checks.reconnect -isnot [bool] -or -not $care[0].checks.reconnect -or $care[0].checks.restart -isnot [bool] -or $care[0].checks.restart) { throw 'Restart verification requires an ordinary same-run reconnect receipt awaiting restart.' }
    $doc=$previous
}
$desktop = $null; $socket = $null
$workers = @{samplingStarted=$false;assessmentStarted=$false;assessmentId='';careStarted=$false;helper=$null}
$fixture=$null
$script:cdpId = 0
$deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)

function Save-Json([string]$Path, $Value) { $Value | ConvertTo-Json -Depth 80 | Set-Content -LiteralPath $Path -Encoding utf8NoBOM }
function Witness([string]$Path) {
    @{path=[IO.Path]::GetRelativePath($evidence,$Path).Replace('\','/');sha256=(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
}
function Cdp([string]$Method, [hashtable]$Params=@{}) {
    if ([DateTime]::UtcNow -ge $deadline) { throw 'Installed acceptance deadline elapsed.' }
    $script:cdpId++
    $cts = [Threading.CancellationTokenSource]::new([TimeSpan]::FromSeconds([Math]::Min(30,($deadline-[DateTime]::UtcNow).TotalSeconds)))
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes((@{id=$script:cdpId;method=$Method;params=$Params} | ConvertTo-Json -Depth 20 -Compress))
        # A non-generic Task is Task<VoidTaskResult> at run time: GetResult() would join the return value.
        $null = $socket.SendAsync([ArraySegment[byte]]::new($bytes),[Net.WebSockets.WebSocketMessageType]::Text,$true,$cts.Token).GetAwaiter().GetResult()
        do {
            $stream = [IO.MemoryStream]::new()
            try {
                do {
                    $buffer = [byte[]]::new(65536)
                    $part = $socket.ReceiveAsync([ArraySegment[byte]]::new($buffer),$cts.Token).GetAwaiter().GetResult()
                    if ($part.MessageType -eq [Net.WebSockets.WebSocketMessageType]::Close) { throw 'WebView2 disconnected.' }
                    $stream.Write($buffer,0,$part.Count)
                    if ($stream.Length -gt 33554432) { throw 'CDP witness exceeds 32 MiB bound.' }
                } until ($part.EndOfMessage)
                $answer = [Text.Encoding]::UTF8.GetString($stream.ToArray()) | ConvertFrom-Json -AsHashtable
            } finally { $stream.Dispose() }
        } until ($answer.ContainsKey('id') -and $answer.id -eq $script:cdpId)
        if ($answer.ContainsKey('error')) { throw ($answer.error | ConvertTo-Json -Compress) }
        return $answer.result
    } finally { $cts.Dispose() }
}
function Js([string]$Expression) {
    $r = Cdp Runtime.evaluate @{expression=$Expression;awaitPromise=$true;returnByValue=$true}
    if ($r.ContainsKey('exceptionDetails')) { throw ($r.exceptionDetails | ConvertTo-Json -Depth 8 -Compress) }
    if ($r.result.ContainsKey('value')) { return $r.result.value }
    return $null
}
function Page([string]$Name) {
    $reached = Js "(async()=>{ const b=document.querySelector('button[data-nav-item][data-page=$Name]'); if(!b) return false; b.focus(); b.click(); await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))); return document.querySelector('.app-shell')?.dataset.page==='$Name'; })()"
    if (-not $reached) { throw "Installed route not reached: $Name" }
}
function Wait-RenderedScan([string]$Selector, [string]$ScanId) {
    if (-not $ScanId) { throw 'An unidentified scan cannot qualify rendered evidence.' }
    $expected=$ScanId | ConvertTo-Json -Compress
    do {
        $rendered=Js "(async()=>{await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return document.querySelector('$Selector')?.dataset.scanId===$expected})()"
        if ($rendered) { return }
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Installed DOM did not render the completed scan identity.' }
        Start-Sleep -Milliseconds 100
    } while ($true)
}
function Capture([string]$Name, [string]$Selector='main') {
    $runtime = Js "(async()=>{const el=document.querySelector('$Selector');if(!el||!el.innerText.trim())throw Error('Actual installed surface is absent or empty');el.scrollIntoView({block:'center',behavior:'instant'});await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));if(!el.isConnected||!el.innerText.trim())throw Error('Installed surface changed before capture');return {url:location.href,tauri:!!window.__TAURI_INTERNALS__,locale:document.documentElement.lang,page:document.querySelector('.app-shell')?.dataset.page,selector:'$Selector',text:el.innerText,focused:document.activeElement?.outerHTML}})()"
    if (-not $runtime.tauri -or $runtime.locale -ne $Locale -or [string]::IsNullOrWhiteSpace($runtime.text)) { throw 'A fixture, absent surface or wrong locale cannot qualify installed UI.' }
    $runtime['service'] = Invoke-Installed get_snapshot
    if (-not $runtime.service.connected) { throw 'Installed UI is disconnected from its service.' }
    $json = Join-Path $capture "$Name-runtime.json"; Save-Json $json $runtime
    $ax = Join-Path $capture "$Name-accessibility.json"; Save-Json $ax (Cdp Accessibility.getFullAXTree)
    $png = Join-Path $capture "$Name.png"
    $shot = Cdp Page.captureScreenshot @{format='png';captureBeyondViewport=$false}
    [IO.File]::WriteAllBytes($png,[Convert]::FromBase64String($shot.data))
    $runtimeWitness=Witness $json;$runtimeWitness['role']='runtime'
    $axWitness=Witness $ax;$axWitness['role']='accessibility'
    $pngWitness=Witness $png;$pngWitness['role']='screenshot'
    $witnesses=@($runtimeWitness;$axWitness;$pngWitness)
    # Care restart replaces only its own surface; all other captured surfaces survive verify mode.
    $surfaces=@{'hardware'='hardware';'repair'='repair';'care'='care';'care-after-restart'='care';'deep-scan'='deep-scan';'timeline'='timeline';'assistant'='assistant'}
    if ($surfaces.ContainsKey($Name)) { $doc.surface_witnesses[$surfaces[$Name]]=$witnesses }
    return $witnesses
}
function Case([int]$Index, [scriptblock]$Run) {
    try { & $Run; if ($doc.cases[$Index].disposition -eq 'blocked' -and $doc.cases[$Index].reason -eq 'Not executed.') { $doc.cases[$Index].disposition='passed';$doc.cases[$Index].reason='Actual installed probe passed.' } }
    catch { $doc.cases[$Index].disposition='failed';$doc.cases[$Index].reason=$_.Exception.Message }
}
function Invoke-CareSmoke([string]$Log, [string[]]$ProcessArgs=@()) {
    $remaining=($deadline-[DateTime]::UtcNow).TotalMilliseconds
    if ($remaining -le 0) { throw 'Installed acceptance deadline elapsed before Care helper.' }
    $start=[Diagnostics.ProcessStartInfo]::new((Join-Path $ReleaseRoot 'acceptance/care_smoke.exe'))
    $start.UseShellExecute=$false;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
    foreach ($argument in $ProcessArgs) { $start.ArgumentList.Add($argument) }
    $workers.helper=[Diagnostics.Process]::Start($start)
    $stdout=$workers.helper.StandardOutput.ReadToEndAsync();$stderr=$workers.helper.StandardError.ReadToEndAsync()
    # A timeout is an observation failure, never permission to kill a mutating worker.
    if (-not $workers.helper.WaitForExit([int][Math]::Min([int]::MaxValue,$remaining))) { throw 'Care helper exceeded the acceptance budget; its worker is preserved.' }
    [IO.File]::WriteAllText($Log,($stdout.GetAwaiter().GetResult()+$stderr.GetAwaiter().GetResult()))
    if ($workers.helper.ExitCode -ne 0) { throw 'Actual installed Care helper failed.' }
}
function Get-OwnedState([string]$Command) {
    try { return Invoke-Installed $Command }
    catch {
        # A user who never started this kind of work owns no state of it: the service answers a
        # typed <domain>.stateUnavailable, not Idle (owner PC run 5, 2026-10-09). Nothing to wait for.
        $value=try { ($_.Exception.Message | ConvertFrom-Json).exception.value } catch { $null }
        if ($value -is [string] -and $value -cmatch '^[A-Za-z]+\.stateUnavailable$') { return $null }
        throw
    }
}
function Wait-Terminal([string]$Command, [string]$ExpectedId='', [string]$IdField='') {
    $terminal = switch ($Command) {
        'get_diagnostics_snapshot' { @('Idle','Ready','Partial','Failed') }
        'get_cleanup_snapshot' { @('Idle','Ready','Failed') }
        'get_repair_assessment' { @('Idle','Ready','Failed','Cancelled') }
        'get_care_status' { @('Idle','AwaitingConsent','Completed','Failed','Cancelled') }
        default { throw 'Unknown provider state contract.' }
    }
    do {
        $state = Get-OwnedState $Command
        if ($null -eq $state) {
            if ($ExpectedId) { throw 'Provider operation is no longer owned; another worker is preserved.' }
            return $null
        }
        if ($ExpectedId -and $state[$IdField] -ne $ExpectedId) { throw 'Provider operation identity changed; another worker is preserved.' }
        if ($state.state -in $terminal) { return $state }
        if ($state.state -notin @('Scanning','Collecting','Running')) { throw 'Unknown provider state cannot prove terminal completion.' }
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Provider did not reach a terminal state within the acceptance budget.' }
        Start-Sleep -Milliseconds 250
    } while ($true)
}
try {
    if ($elevated -or $os.ProductType -ne 1 -or $os.Caption -notlike '*Windows 11*' -or [Diagnostics.Process]::GetCurrentProcess().SessionId -eq 0) { throw 'Windows 11 ordinary interactive desktop required.' }
    $installed = Join-Path $env:ProgramFiles 'AetherCore'
    $exe = Join-Path $installed 'aethercore-desktop.exe'
    if (Get-Process aethercore-desktop -ErrorAction SilentlyContinue) { throw 'Existing desktop session is preserved; use a fresh acceptance session.' }
    if (-not $ReadOnlyInstalled) {
        $metadata=Get-Content (Join-Path $ReleaseRoot 'RELEASE-METADATA.json') -Raw | ConvertFrom-Json -AsHashtable
        if ($metadata.source_commit -ne $ExpectedSourceSha) { throw 'RC metadata belongs to another source commit.' }
        $bundles = @(Get-ChildItem (Join-Path $ReleaseRoot 'artifacts') -Filter 'AetherCoreSetup-*.exe')
        if ($bundles.Count -ne 1 -or (Get-FileHash $bundles[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant() -ne $ExpectedBundleSha256) { throw 'Bundle does not match the pinned RC.' }
        $payload=@(Get-ChildItem (Join-Path $ReleaseRoot 'payload') -Filter '*.exe')
        foreach ($name in @('aethercore-desktop.exe','aethercore-maintenance-service.exe','aethercore-consent-broker.exe','aethercore-update-broker.exe','aethercore-install-hardener.exe','aetherctl.exe')) {
            if ($name -notin $payload.Name) { throw "Required RC payload is missing: $name" }
        }
        foreach ($file in $payload) {
            if ((Get-FileHash $file.FullName -Algorithm SHA256).Hash -ne (Get-FileHash (Join-Path $installed $file.Name) -Algorithm SHA256).Hash) { throw "Installed bytes differ from RC: $($file.Name)" }
        }
        if (-not $VerifyCareRunId) {
            $tempRoot=Join-Path $env:SystemRoot 'Temp'
            if ((Get-Item -LiteralPath $tempRoot).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Disposable cleanup fixture root is a reparse point.' }
            $fixture=Join-Path $tempRoot ("aethercore-p87-$([Guid]::NewGuid()).tmp")
            $stream=[IO.File]::Open($fixture,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
            try { $bytes=[Text.Encoding]::UTF8.GetBytes('AetherCore disposable installed acceptance fixture.');$stream.Write($bytes,0,$bytes.Length) } finally { $stream.Dispose() }
            $old=[DateTime]::UtcNow.AddDays(-3)
            [IO.File]::SetCreationTimeUtc($fixture,$old);[IO.File]::SetLastWriteTimeUtc($fixture,$old);[IO.File]::SetLastAccessTimeUtc($fixture,$old)
            $doc['fixture']=@{path=$fixture;sha256=(Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash.ToLowerInvariant();bytes=$bytes.Length;disposable=$true}
        }
    }
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0); $listener.Start()
    $port = $listener.LocalEndpoint.Port; $listener.Stop()
    $start = [Diagnostics.ProcessStartInfo]::new($exe); $start.UseShellExecute=$false
    # Microsoft-supported test-process debugging; no registry or product defaults change.
    $start.Environment['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS']="--remote-debugging-port=$port --remote-debugging-address=127.0.0.1"
    $desktop = [Diagnostics.Process]::Start($start)
    $target = $null
    do {
        try { $targets = @(Invoke-RestMethod "http://127.0.0.1:$port/json/list" -TimeoutSec 2); $target = $targets | Where-Object { $_.type -eq 'page' -and $_.url -match '^(?:tauri://localhost/|https?://tauri\.localhost/)' } | Select-Object -First 1 } catch { }
        if ($desktop.HasExited) { throw 'Installed desktop exited before its WebView2 became available.' }
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Installed WebView2 debugging endpoint unavailable.' }
        if (-not $target) { Start-Sleep -Milliseconds 250 }
    } until ($target)
    $uri = [Uri]$target.webSocketDebuggerUrl
    if ($uri.Host -notin @('127.0.0.1','localhost') -or $uri.Port -ne $port) { throw 'Debug target is not the owned loopback endpoint.' }
    $socket = [Net.WebSockets.ClientWebSocket]::new()
    $cts = [Threading.CancellationTokenSource]::new([TimeSpan]::FromSeconds(10))
    try { $null = $socket.ConnectAsync($uri,$cts.Token).GetAwaiter().GetResult() } finally { $cts.Dispose() }
    [void](Cdp Page.enable)
    [void](Js "localStorage.setItem('aethercore.locale','$Locale'); location.reload(); true")
    do {
        Start-Sleep -Milliseconds 100
        try { $ready = Js "(async()=>{if(document.documentElement.lang!=='$Locale'||!document.querySelector('.app-shell')||!window.__TAURI_INTERNALS__)return false;return (await window.__TAURI_INTERNALS__.invoke('get_snapshot')).connected===true})()" }
        catch { $ready=$false }
    } until ($ready -or [DateTime]::UtcNow -ge $deadline)
    if (-not $ready) { throw 'Installed locale did not settle.' }
    if ($VerifyCareRunId) {
        $log=Join-Path $capture 'care-after-service-restart.txt'
        Invoke-CareSmoke $log @('--verify-run-id',$VerifyCareRunId)
        if (-not (Select-String -LiteralPath $log -SimpleMatch "SMOKE: persisted-care-run=$VerifyCareRunId" -Quiet)) { throw 'Care run was not recovered through a new installed IPC connection after service restart.' }
        Page activity
        $care[0].witnesses+=@(Witness $log);$care[0].witnesses+=Capture care-after-restart '.care-panel'
        $care[0].checks.restart=$true
        $care[0].disposition='passed';$care[0].reason='Exact Care run survives independent IPC reconnect and service restart.'
        $doc.restart_pending=$false
    } else {
    Case 0 {
        Page hardware
        if (-not $ReadOnlyInstalled) { $started=Invoke-Installed start_diagnostics_scan; $state=Wait-Terminal get_diagnostics_snapshot $started.scanId scanId;Wait-RenderedScan '.storage-grid' $state.scanId }
        $text = @(Js "Array.from(document.querySelectorAll('.storage-head>div>p:not(.eyebrow),.disk-missing')).map(x=>x.innerText)")
        Assert-OwnedText $text $Locale
        $doc.cases[0].witnesses=Capture hardware '.storage-grid'
    }
    Case 1 {
        Page performance
        if (-not $ReadOnlyInstalled) { [void](Invoke-Installed start_perf_sampling @{intervalMs=1000});$workers.samplingStarted=$true;Start-Sleep -Milliseconds 1200 }
        $text=@(Js "Array.from(document.querySelectorAll('.warning-strip p')).map(x=>x.innerText)")
        if (-not $text.Count) { $doc.cases[1].reason='No unavailable/degraded collector was exercised.' } else { Assert-OwnedText $text $Locale;$doc.cases[1].disposition='passed';$doc.cases[1].reason='Observed provider faults have owned labels.' }
        $doc.cases[1].witnesses=Capture performance '.warning-strip'
        if ($workers.samplingStarted) { [void](Invoke-Installed stop_perf_sampling);$workers.samplingStarted=$false }
    }
    Case 2 {
        Page cleanup
        if (-not $ReadOnlyInstalled) { $started=Invoke-Installed start_cleanup_scan;$state=Wait-Terminal get_cleanup_snapshot $started.scanId scanId;Wait-RenderedScan '.cleanup-list' $state.scanId }
        $text=@(Js "Array.from(document.querySelectorAll('.cleanup-copy strong,.cleanup-copy p,.cleanup-copy small')).map(x=>x.innerText)")
        Assert-OwnedText $text $Locale
        $doc.cases[2].witnesses=Capture cleanup '.cleanup-list'
    }
    Case 3 {
        if ($ReadOnlyInstalled) { $doc.cases[3].reason='Care mutation and persistence restart are not executed on the owner host.';return }
        $log = Join-Path $capture 'care-smoke.txt'
        if ((Invoke-Installed get_care_status).state -eq 'Running') { throw 'Existing Care worker is preserved.' }
        $workers.careStarted=$true
        Invoke-CareSmoke $log
        $doc.cases[3].witnesses=@(Witness $log)
        if ($fixture -and (Test-Path -LiteralPath $fixture)) { throw 'The acknowledged cleanup did not delete its actual test fixture.' }
        $lines=Get-Content -LiteralPath $log
        $runLine=@($lines | Where-Object { $_ -cmatch '^SMOKE: care-run-id=care-[0-9]{1,19}$' })
        if ($runLine.Count -ne 1) { throw 'Care smoke omitted its unique run identity.' }
        $doc['care_run_id']=$runLine[0].Substring('SMOKE: care-run-id='.Length)
        if (-not (Select-String -LiteralPath $log -SimpleMatch "SMOKE: reconnected-care-run=$($doc.care_run_id)" -Quiet)) { throw 'Independent Care reconnect was not proved.' }
        Page activity
        $doc.cases[3].witnesses+=Capture care '.care-panel'
        $doc.cases[3].checks=@{reconnect=$true;restart=$false}
        $doc['restart_pending']=$true
        $doc.cases[3].reason='Independent reconnect passed; disposable lifecycle service-restart proof is pending.'
    }
    Case 4 {
        Page repair
        if ($ReadOnlyInstalled) { $doc.cases[4].reason='Assessment/cancellation is not started on the owner host.';return }
        $existing=Get-OwnedState get_repair_assessment
        if ($null -ne $existing -and $existing.state -eq 'Scanning') { throw 'Existing assessment worker is preserved.' }
        $workers.assessmentStarted=$true
        $assessment=Invoke-Installed start_repair_assessment;$workers.assessmentId=$assessment.assessmentId
        $progressDeadline=[DateTime]::UtcNow.AddSeconds(20)
        while ($assessment.state -eq 'Scanning' -and -not $assessment.currentCheckId -and $assessment.checks.Count -eq 0 -and [DateTime]::UtcNow -lt $progressDeadline -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 100
            $assessment=Invoke-Installed get_repair_assessment
            if ($assessment.assessmentId -ne $workers.assessmentId) { throw 'Assessment identity changed; another worker is preserved.' }
        }
        $before = Join-Path $capture 'repair-before-cancel.json';Save-Json $before $assessment
        $doc.cases[4].witnesses=@(Witness $before)
        $doc.cases[4].witnesses+=Capture repair-progress
        if ($assessment.state -eq 'Scanning') { [void](Invoke-Installed cancel_repair_assessment @{assessmentId=$assessment.assessmentId}) }
        $terminal=Wait-Terminal get_repair_assessment $workers.assessmentId assessmentId;$workers.assessmentStarted=$false
        if ($terminal.state -notin @('Ready','Failed','Cancelled')) { throw 'Assessment did not settle to a declared terminal state.' }
        $after=Join-Path $capture 'repair-terminal.json';Save-Json $after $terminal
        $doc.cases[4].witnesses+=@(Witness $after);$doc.cases[4].witnesses+=Capture repair
        $unavailable=@($terminal.checks | Where-Object { $_.resultCode -match 'Unavailable|NotAvailable|Unsupported|ProviderFailure' }).Count -gt 0
        $progressObserved=([bool]$assessment.currentCheckId -or $assessment.checks.Count -gt 0)
        $doc.cases[4].checks=@{terminal=$true;unavailable_provider=$unavailable;progress=$progressObserved}
        if ($OwnerHostAccepted -and $progressObserved -and -not $unavailable) {
            # D34: a healthy owner PC has no unavailable provider to observe, and a full assessment
            # outlasts the budget. Name it as not observed; never claim it.
            $doc.cases[4].checks['unavailable_provider_owner_decision']='D34'
            $doc.cases[4].reason='Progress and terminal observed. Unavailable provider: not observed on the owner''s healthy machine, by owner decision D34.'
            $doc.cases[4].disposition='passed'
        } elseif (-not $unavailable -or -not $progressObserved) { $doc.cases[4].reason='Terminal/cancellation observed; progress or unavailable-provider fixture was not exercised.' }
    }
    Case 5 {
        if (-not $ReadOnlyInstalled) {
            # The completed Care cleanup leaves its old Ready inventory intact. Re-measure
            # before preparing a no-op; never replace an already active cleanup worker.
            $existing=Invoke-Installed get_cleanup_snapshot
            if ($existing.state -eq 'Scanning') { throw 'Existing cleanup worker is preserved; no-op qualification is blocked.' }
            $started=Invoke-Installed start_cleanup_scan
            if ([string]::IsNullOrWhiteSpace($started.scanId)) { throw 'Fresh cleanup scan identity was not reported.' }
            $scan=Wait-Terminal get_cleanup_snapshot $started.scanId scanId
            if ($scan.state -ne 'Ready') { throw 'Fresh owned cleanup inventory did not complete successfully.' }
            Page overview
            [void](Js "(()=>{const b=document.querySelector('.overview-header-side button.secondary');if(!b||b.disabled)throw Error('Actual Care preparation control unavailable');b.focus();b.click();return true})()")
        } else {
            $scan=Invoke-Installed get_cleanup_snapshot
        }
        Page activity
        $eligible=@($scan.candidates | Where-Object { $_.selectedByDefault -and -not $_.requiresExplicitConfirmation })
        if ($scan.state -ne 'Ready' -or $eligible.Count -ne 0) { $doc.cases[5].reason='Native completed scan with no automatic candidates was not exercised.' }
        else {
            $expected = if ($Locale -eq 'en') { 'Nothing eligible for automatic care was found: the default cleanup category had nothing to clean.' } else { 'لم يُعثر على شيء مؤهل للعناية التلقائية: فئة التنظيف الافتراضية لم يكن فيها ما يُنظَّف.' }
            do {
                $empty=@(Js "Array.from(document.querySelectorAll('.care-panel p')).map(x=>x.innerText)")
                if ($empty -contains $expected) { break }
                if ([DateTime]::UtcNow -ge $deadline) { throw 'Actual no-op Care explanation did not appear.' }
                Start-Sleep -Milliseconds 100
            } while ($true)
            Assert-OwnedText $empty $Locale;$doc.cases[5].checks=@{no_op_explained=$true};$doc.cases[5].disposition='passed';$doc.cases[5].reason='A completed scan with no automatic candidates has the owned no-op explanation.'
        }
        $doc.cases[5].witnesses=Capture care-eligibility '.care-panel'
    }
    # Capture all ASTRA P87-02A surfaces without starting a model question or another scan.
    Page overview
    $deepScan=Js "(async()=>{const b=document.querySelector('.overview-header-side button.primary');if(!b)throw Error('Actual DeepScan navigation control missing');b.focus();b.click();await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return document.querySelector('.app-shell')?.dataset.page==='deepScan'})()"
    if (-not $deepScan) { throw 'Actual DeepScan route was not reached.' }
    [void](Capture deep-scan '.deep-scan-header')
    Page activity
    [void](Capture timeline '.timeline-panel')
    $assistant=Js "(async()=>{const b=document.querySelector('button.assistant-nav-item');if(!b)throw Error('Actual assistant control missing');b.focus();b.click();await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return !!document.querySelector('.assistant-drawer')})()"
    if (-not $assistant) { throw 'Actual installed assistant drawer did not open.' }
    do {
        $packSettled=Js "(()=>{const e=document.querySelector('.assistant-engine');return !!e&&!!e.innerText.trim()&&e.innerText.trim()!=='—'&&!!document.querySelector('.assistant-transcript')})()"
        if ($packSettled) { break }
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Actual assistant evidence pack did not settle.' }
        Start-Sleep -Milliseconds 100
    } while ($true)
    [void](Capture assistant '.assistant-drawer')
    # Supplementary native screenshots/AX: keyboard activation and 200% scale are recorded,
    # not silently promoted to human Narrator qualification.
    [void](Cdp Input.dispatchKeyEvent @{type='keyDown';key='Tab';code='Tab';windowsVirtualKeyCode=9})
    [void](Cdp Input.dispatchKeyEvent @{type='keyUp';key='Tab';code='Tab';windowsVirtualKeyCode=9})
    [void](Cdp Emulation.setPageScaleFactor @{pageScaleFactor=2})
    $doc['supplementary_witnesses']=Capture keyboard-scale
    }
} catch {
    $doc['blocked_reason']=$_.Exception.Message
} finally {
    $doc['worker_ownership_released']=($null -ne $socket)
    if ($socket) {
        # Cleanup has its own bounded drain; it does not turn an expired test into a pass.
        $deadline=[DateTime]::UtcNow.AddSeconds(30)
        if ($workers.samplingStarted) { try { [void](Invoke-Installed stop_perf_sampling) } catch { $doc['sampling_cleanup_error']=$_.Exception.Message;$doc.worker_ownership_released=$false } }
        if ($workers.assessmentStarted) {
            try {
                $remaining=Invoke-Installed get_repair_assessment
                if ($remaining.state -eq 'Scanning') {
                    if (-not $workers.assessmentId -or $remaining.assessmentId -ne $workers.assessmentId) { throw 'Assessment identity is unconfirmed; worker is preserved.' }
                    [void](Invoke-Installed cancel_repair_assessment @{assessmentId=$workers.assessmentId})
                    [void](Wait-Terminal get_repair_assessment $workers.assessmentId assessmentId)
                }
            } catch { $doc['assessment_cleanup_error']=$_.Exception.Message;$doc.worker_ownership_released=$false }
        }
        if ($workers.careStarted) {
            try { [void](Wait-Terminal get_care_status) }
            catch { $doc['care_cleanup_error']=$_.Exception.Message;$doc.worker_ownership_released=$false }
        }
        if (-not $ReadOnlyInstalled) {
            try {
                foreach ($command in @('get_repair_assessment','get_care_status','get_diagnostics_snapshot','get_cleanup_snapshot')) { [void](Wait-Terminal $command) }
            } catch { $doc['active_worker_error']=$_.Exception.Message;$doc.worker_ownership_released=$false }
        }
        $socket.Dispose()
    } elseif ($workers.assessmentStarted -or $workers.careStarted -or $workers.samplingStarted) {
        $doc.worker_ownership_released=$false
    }
    $doc['desktop_closed']=$true
    if ($desktop -and -not $desktop.HasExited) {
        [void]$desktop.CloseMainWindow()
        if (-not $desktop.WaitForExit(10000)) {
            $doc.desktop_closed=$false;$doc.worker_ownership_released=$false
            $doc['desktop_cleanup_error']='Owned test desktop did not exit; lifecycle teardown is blocked.'
        }
    }
    if ($workers.helper -and -not $workers.helper.HasExited) {
        $doc.worker_ownership_released=$false
        $doc['helper_cleanup_error']='Care helper is still alive; lifecycle teardown is blocked.'
    }
    if ($fixture -and $doc.worker_ownership_released -and (Test-Path -LiteralPath $fixture)) {
        try {
            $item=Get-Item -LiteralPath $fixture
            if ($doc.fixture.path -ne $fixture -or $doc.fixture.disposable -isnot [bool] -or -not $doc.fixture.disposable -or $item.Attributes -band [IO.FileAttributes]::ReparsePoint -or $item.Length -ne $doc.fixture.bytes -or (Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash.ToLowerInvariant() -ne $doc.fixture.sha256) { throw 'Owned disposable fixture changed; it is preserved.' }
            Remove-Item -LiteralPath $fixture
        } catch { $doc['fixture_cleanup_error']=$_.Exception.Message;$doc.worker_ownership_released=$false }
    }
    $doc['ok']=(-not $ReadOnlyInstalled -and $doc.worker_ownership_released -and -not $doc.Contains('blocked_reason') -and @($doc.cases | Where-Object disposition -ne 'passed').Count -eq 0)
    Save-Json ([IO.Path]::GetFullPath($OutputPath)) $doc
}
if (-not $doc.ok) { Write-Host 'P87 installed acceptance: BLOCKED/FAILED (see individual dispositions).';exit 1 }
Write-Host 'P87 installed acceptance: PASS'
