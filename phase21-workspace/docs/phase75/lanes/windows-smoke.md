# P75 lane `windows-smoke` (part 3, trial run) — evidence (`DBT-P75-076`)

`windows-installer.yml` `bundle-log-acl-probe` now uses the product it installs, before it
measures removal: the service must reach RUNNING within 150 s; 14 read-only aetherctl verbs
(capabilities, engine-source, self-check, doctor, perf start/snapshot/report/stop, scan
start/status, care status, insights list, timeline page/patterns) must exit 0 with `"ok":true`
through the installed aetherctl; and `care_smoke.exe` (`crates/ipc/examples/care_smoke.rs`,
built by `build-unsigned-candidate`) drives One-Click Care over the named pipe.

| run | tree | result |
|---|---|---|
| `36289309971` | `main` + smoke (branch `probe/p75-smoke-red`) | red: care → "operation engine: authorization required" (`DBT-P75-045`); doctor exit 5 (`DBT-P75-063`) |
| `36289311454` | care lane + smoke | care PASS (Cleanup VerifiedByDomain; second start AwaitingConsent); doctor exit 5 |
| `36338076643` | final main + smoke | 14/14 verbs; care refused the smoke's empty grant (`care.error.planChanged`, `DBT-P75-081`) — the client was wrong |
| `36341345870` | this lane | green: 14/14 verbs; scan → plan → preview → grant (previewed digest) → run: Cleanup VerifiedByDomain in 135 ms; a grant for a plan never shown refused; a second start AwaitingConsent; `GATE: pass` |

Evidence files in the run's artifact: `smoke-aetherctl.txt`, `smoke-care.txt`.
