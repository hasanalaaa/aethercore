# Stage 3 (partial) - the OFFLINE, strictly read-only aetherctl surface, run from
# the freshly built x64 binary against real silicon. No service, no elevation.
$ErrorActionPreference = 'Continue'
$sp  = 'C:\Users\husen\AppData\Local\Temp\claude\C--dev-aethercore\1b2c52d3-134a-4b87-a721-5281ab2187d1\scratchpad'
$log = "$sp\offline-verbs.log"
$ctl = 'C:\dev\aethercore\phase21-workspace\target\release\aetherctl.exe'
$vdir = "$sp\offline"; New-Item -ItemType Directory -Force $vdir | Out-Null
function W($s){ Add-Content -Path $log -Value $s -Encoding UTF8 }
Set-Content -Path $log -Value ("OFFLINE VERBS " + (Get-Date -Format o)) -Encoding UTF8
W ("CTL=" + $ctl)
W ("CTL_SHA256=" + (Get-FileHash $ctl -Algorithm SHA256).Hash.ToLower())
W ("WHOAMI=" + (whoami))
W ("ELEVATED=" + ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator))

function Run-Verb([string]$label,[string[]]$argv,[int]$timeoutMs = 120000){
  W ("=== VERB " + $label + " : aetherctl " + ($argv -join ' ') + " ===")
  $so = "$vdir\$label.out"; $se = "$vdir\$label.err"
  $sw = [Diagnostics.Stopwatch]::StartNew()
  try {
    $p = Start-Process -FilePath $ctl -ArgumentList $argv -PassThru -NoNewWindow -RedirectStandardOutput $so -RedirectStandardError $se -WorkingDirectory 'C:\dev\aethercore\phase21-workspace'
    $null = $p.Handle
    $done = $p.WaitForExit($timeoutMs)
    $sw.Stop()
    if ($done) { W ("RESULT=RETURNED"); W ("EXIT_CODE=" + $p.ExitCode) } else { try{$p.Kill()}catch{}; W ("RESULT=TIMED_OUT_" + $timeoutMs + "MS") }
  } catch { $sw.Stop(); W ("RESULT=LAUNCH_FAILED: " + $_.Exception.Message) }
  W ("ELAPSED_MS=" + $sw.ElapsedMilliseconds)
  $o = Get-Content $so -Raw -EA SilentlyContinue
  $e = Get-Content $se -Raw -EA SilentlyContinue
  if ($o) { W ("STDOUT:`n" + $o.Trim()) } else { W "STDOUT: (empty)" }
  if ($e) { W ("STDERR:`n" + $e.Trim()) }
}

Run-Verb 'version'       @('--output','json','version')
Run-Verb 'about'         @('--output','json','about')
Run-Verb 'capabilities'  @('--output','json','capabilities')
Run-Verb 'enginesource'  @('--output','json','engine-source')
Run-Verb 'servicedetect' @('--output','json','service','detect')
Run-Verb 'telemetryonce' @('--output','json','telemetry-once')
Run-Verb 'selfcheck'     @('--output','json','self-check')
Run-Verb 'selfcheckload' @('--output','json','self-check','--load-model') 300000
W "OFFLINE_DONE"
