# P75 lane `diagnosis-evidence` — evidence (ledger `DBT-P75-018`…`023`)

Branch `lane/diagnosis-evidence`. Scope: `crates/crash-diagnostics/**`,
`crates/diagnostic-engine/**`, `crates/performance-bottleneck/**`. No dependency,
lockfile or threshold change. Each finding is from the P75 audit (`AMBITION.md`
§2, "high", MEASURED).

| row | commit | the claim that was wrong | now | red-before |
|---|---|---|---|---|
| `DBT-P75-018` | `0fd2af9` | a failed Event Log read rendered "no logged memory hardware errors … in the last 30 days" | `event_log_read` is false when `EvtQuery` fails, and all three card paths say unavailable | `failed_event_log_read_is_unavailable_on_every_path` failed on `f22d285` |
| `DBT-P75-019` | `0fd2af9` | a full scan with provider faults and no warning text said Ready | Ready needs no warnings and no provider faults | `full_scan_with_provider_faults_is_not_ready` failed on `f22d285` |
| `DBT-P75-020` | `2da7c0c` | Kernel-Power events of every id filled the 128-event cap, though only id 41 is classified | the XPath takes Kernel-Power `EventID=41` only | windows-2025 throwaway run `36186229655`: the old query returned 9 events, all Kernel-Power 109/172/577 and none 41; the Event Log accepts the new query (`NoMatchingEventsFound`) |
| `DBT-P75-021` | `57db03b` | minidumps were never age-filtered under a "last 30 days" card | dumps outside the window, or without a readable time, are left out, and the count is reported as a warning | `minidumps_outside_the_window_or_without_a_time_are_left_out` (tests a new helper, so red by construction) |
| `DBT-P75-022` | `4d0e140` | one GPU sample ≥ 94 % made `GPU_BOUND_WORKLOAD` the root cause | decided on the average of reporting samples; the peak is still cited | `a_single_gpu_spike_is_not_a_root_cause` failed (`["GPU_BOUND_WORKLOAD"]`) |
| `DBT-P75-023` | `d3ae9a9`, `c59ee72` | an I/O root-cause card could appear with no storage sample, and it dated an earlier peak at the last snapshot | peak and average come from current storage readings; the evidence carries the peak's own timestamp | two new attribution checks failed on the unchanged rule (11 passed, 2 failed) |

## Local verification (macOS, from `phase21-workspace/`)

* `cargo test -p aethercore-crash-diagnostics -p aethercore-performance-bottleneck --locked`: 7 + 14 pass.
* `cargo clippy -p <each> --all-targets --locked -- -D warnings`, host and
  `--target x86_64-pc-windows-msvc`: clean for both crates.
* The live Windows Event Log/minidump test stays `#[ignore]`. The XPath was run
  on the runner as recorded above; the Windows CI job is the compile and unit
  verdict.
