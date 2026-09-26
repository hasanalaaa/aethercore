# P75 lane `telemetry-windows` — evidence (ledger `DBT-P75-028`, `DBT-P75-029`)

Branch `lane/telemetry-windows`. Scope: `crates/performance-telemetry/src/{windows_impl,lib}.rs`
and its tests. No dependency or lockfile change (`collector-runtime` was already a dependency).

| row | commit | what was wrong | now | red-before |
|---|---|---|---|---|
| `DBT-P75-028` | `15fbaed` | `stop()` then `start()` inside one interval left the old sampler thread running beside the new one | the ring keeps the running sampler's generation; a thread exits once it is not its own, and re-checks under the ring lock before pushing | `stop_then_start_within_an_interval_leaves_exactly_one_sampler` failed 3/3 ("1 calls at restart, 5 after 1.1 s") |
| `DBT-P75-029` | `17a0d8e` | the module promised every collector ran under the runtime timeout; none did, so one hung platform call stalled the sampler forever with an empty ring | every tick runs under `run_isolated_gated` (window + `DEFAULT_COLLECTOR_TIMEOUT`); a fault publishes every subsystem as unavailable and claims no window | `a_hung_collector_becomes_a_timeout_fault_not_a_stalled_sampler` failed on `15fbaed`: "no snapshot after 15 s" |

## Not done in this lane (stays open)

The Windows counter items in `AMBITION.md` §3 need Windows-runner measurement and were not
started: `DBT-P49-004` (`intervalMs` equals the window measured), `DBT-P47-003` (per-processor via
PDH wildcard), GPU per-process versus per-adapter, fault counters (paging counters today),
`MhzLimit` units, letterless-disk capacity, `% Disk Time` versus `% Idle Time`. Their ledger rows
are unchanged.

## Local verification (macOS)

* `cargo test -p aethercore-performance-telemetry --locked`: all pass (lifecycle 3/3).
* `cargo clippy -p aethercore-performance-telemetry --all-targets --locked -- -D warnings`: clean;
  `--lib --target x86_64-pc-windows-msvc`: clean.
