# Runs AS the probe principal (one-shot Scheduled Task). Exercises the four
# typed verbs and records RETURNED vs TIMED_OUT for each.
$ErrorActionPreference = 'Continue'
$ctx = $env:P36_CTX; if (-not $ctx) { $ctx = 'unknown' }
$dir = 'C:\Users\Public\p36'
New-Item -ItemType Directory -Force $dir | Out-Null
$out = Join-Path $dir ("verbs-" + $ctx + ".txt")
# P37 Stage 1: aetherctl is now an authored MSI component, so the INSTALLED copy is
# what a real user gets and is what must be exercised. The staged tools copy stays
# as a fallback so the Phase 36 evidence path still runs on a bare box.
$ctl = 'C:\Program Files\AetherCore\aetherctl.exe'
if (-not (Test-Path $ctl)) { $ctl = 'C:\AetherCore-P36\tools\aetherctl.exe' }
("=== P36 VERB RUN (context: " + $ctx + ") ===") | Set-Content $out
("CAPTURED_UTC=" + (Get-Date).ToUniversalTime().ToString('o')) | Add-Content $out
("WHOAMI=" + (whoami)) | Add-Content $out
$id = [Security.Principal.WindowsIdentity]::GetCurrent()
$pr = New-Object Security.Principal.WindowsPrincipal($id)
("IS_ELEVATED_ADMIN=" + $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) | Add-Content $out
("INTEGRITY=" + (@($id.Groups | Where-Object { $_.Value -like 'S-1-16-*' } | ForEach-Object { $_.Value }) -join ',')) | Add-Content $out
("CTL_PATH=" + $ctl) | Add-Content $out
if (Test-Path $ctl) { ("CTL_SHA256=" + (Get-FileHash $ctl -Algorithm SHA256).Hash.ToLower()) | Add-Content $out }
else { "CTL_MISSING=true" | Add-Content $out }

function Run-Verb([string]$label, [string[]]$argv) {
    ("--- VERB " + $label + " ---") | Add-Content $out
    $so = Join-Path $dir ("o-" + $ctx + "-" + $label + ".out")
    $se = Join-Path $dir ("o-" + $ctx + "-" + $label + ".err")
    $sw = [Diagnostics.Stopwatch]::StartNew()
    try {
        $p = Start-Process -FilePath $ctl -ArgumentList $argv -PassThru -NoNewWindow `
             -RedirectStandardOutput $so -RedirectStandardError $se
        # Cache the process handle BEFORE the process exits, or ExitCode is never
        # populated and every transcript records `EXIT_CODE=` (empty) -- the harness
        # gap recorded in SESSION_CONTEXT §16.4. Windows PowerShell 5.1's
        # Start-Process -PassThru hands back a Process object that has not opened a
        # handle; once the process has exited the kernel object is gone and the exit
        # code is unrecoverable. Touching .Handle here opens and caches it.
        # Measured on the VM (PSVersion 5.1.26100.9168) against known exit codes
        # 0/3/8: without this line all three read back empty, with it all three read
        # back correctly. Adding a parameterless WaitForExit() alone does NOT fix it.
        $null = $p.Handle
        $done = $p.WaitForExit(15000)
        $sw.Stop()
        if ($done) { "RESULT=RETURNED" | Add-Content $out; ("EXIT_CODE=" + $p.ExitCode) | Add-Content $out }
        else { try { $p.Kill() } catch {}; "RESULT=TIMED_OUT_15S" | Add-Content $out }
    } catch { $sw.Stop(); ("RESULT=LAUNCH_FAILED: " + $_.Exception.Message) | Add-Content $out }
    ("ELAPSED_MS=" + $sw.ElapsedMilliseconds) | Add-Content $out
    "STDOUT:" | Add-Content $out; (Get-Content $so -Raw -EA SilentlyContinue) | Add-Content $out
    "STDERR:" | Add-Content $out; (Get-Content $se -Raw -EA SilentlyContinue) | Add-Content $out
}
Run-Verb 'servicedetect'  @('service','detect')
Run-Verb 'doctor'         @('doctor')
Run-Verb 'optimizestatus' @('optimize','status')
Run-Verb 'scanstatus'     @('scan','status')
# P37 Stage 1 additions. These are the verbs that prove the newly authored files
# actually landed: self-check reads assets\models\models.manifest.json and hashes
# the gguf against the pin; sec audit needs the CIS profile (now compiled in) and
# reads assets\vulndb beside the executable; help is the Stage 3 first experience.
Run-Verb 'helpflag'       @('--help')   # Stage 3: currently an UNKNOWN COMMAND, recorded not assumed
Run-Verb 'help'           @('help')
Run-Verb 'selfcheck'      @('self-check')
# engineLabel is the ONLY direct proof that the installed gguf verified and loaded:
# localModel = the embedded reasoner is active, ruleFallback = it is not.
Run-Verb 'insightslist'   @('--output','json','insights','list')
Run-Verb 'secaudit'       @('sec','audit','--profile','cis-l1','--out',(Join-Path $dir ("sec-$ctx.json")),'--format','json')
"=== END ===" | Add-Content $out
