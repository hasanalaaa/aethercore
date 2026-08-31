# Interactive desktop and UAC qualification — 2026-08-31

SESSIONS_FOUND=6 (`P36StandardUser`, Interactive/Active after `tscon`; session 7 administrator disconnected)
DESKTOP_LAUNCHED=PASS — responsive `aethercore-desktop.exe` in session 6; screenshot evidence: `/Users/hasanalaaa/dev/p36-stage/out/desktop-launch.png`
DESKTOP_USABLE=PASS — shell rendered; service verbs returned under the standard-user token.
ENGINE_LABEL_IN_APP=localModel (JSON `insights list` in `verbs-desktop-STD.txt`)
TOKEN_CONTEXT_PROVEN= genuine standard user: `IsInRole(Administrator)=False`, BUILTIN\Users, Medium Mandatory Level (`S-1-16-8192`)
UAC_BEHAVIOUR=NOT EXERCISED — remaining elevation request is human-gated.
UAC_CAVEAT_RECORDED=YES — VM has non-default `PromptOnSecureDesktop=0`; it must accompany every future UAC result.
CREDENTIALS_HANDLED=none
TEMP_ARTIFACTS_REMOVED=YES — scheduled task removed; VM `C:\Users\Public\p36` contains zero `.ps1` files.
STILL_HUMAN_GATED=UAC consent interaction only.
PUSHED=pending this commit
DENIAL_RESULTS_REVERIFIED=YES — service exe write, trust-file write, install-dir create, mutation-lock write, `sc stop`, and `sc config` all still denied (0x5/exit 5).
GROUP_CHANGE_RECORDED=YES — `Add-LocalGroupMember -Group Users -Member P36StandardUser` recorded in `SESSION_CONTEXT.md`.
