# P75 lane `telemetry-trial` (part 3, trial run) — evidence

Found by the unix service on this Mac (`aetherctl perf snapshot|report`, IPC `GetPerformanceSnapshot`), compared with `df`, `iostat`, `memory_pressure` and `sysctl kern.memorystatus_level`.

| row | commit | exposed by | what was wrong | now | red-before |
|---|---|---|---|---|---|
| `DBT-P75-066` | `783179a` | `aetherctl perf snapshot`, `perf report` | macOS put used disk capacity in `active_time_bp` (9043 = 90% full); Disk activity read 90% busy and IO_SATURATION fired (high, rootCause) on an idle disk | active time 0 + `storage.activeTime` fault (macOS and Linux) | `disk_active_time_is_not_capacity`: `left: 9071, right: 0` |
| `DBT-P75-067` | `3e72aaa` | `aetherctl perf snapshot` vs `memory_pressure` | macOS memory load 98% (free + min(inactive, purgeable)); the kernel said 64% free | load from `kern.memorystatus_level` | `memory_load_is_the_kernels_not_free_pages`: "published 93% load, the kernel says 37%" |
| `DBT-P75-068` | `91b51ee` | `aetherctl perf report` | IO_SATURATION fired by the 75% average cited only the peak (9043 of threshold 9200) | the average is cited when it fired the rule | `io_saturation_fired_by_the_average_cites_the_average` |

Local proof: `cargo test -p aethercore-performance-telemetry -p aethercore-performance-bottleneck` pass; clippy `-D warnings` clean on both; `phase20`/`phase21` audits PASS; `phase27` only its pre-existing `p27-wirefreeze` failures.
