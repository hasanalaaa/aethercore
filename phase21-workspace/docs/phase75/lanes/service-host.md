# P75 lane `service-host` (Wave 2) — evidence (ledger `DBT-P60-003`, `DBT-P75-056`)

Branch `lane/service-host`. Written by the fourth lead session from the lane's commits; the
lane itself left no evidence file.

| row | commit | what changed | proof |
|---|---|---|---|
| `DBT-P60-003` | `4301950` | `data_root.rs`: a non-Windows host with no `--data-dir` / `$AETHERCORE_DATA_DIR` fails with "no data directory" instead of writing under `./C:\ProgramData`; unix integration tests pass their own directory | red-before `a_posix_host_never_gets_a_windows_path`: "resolved to C:\ProgramData/AetherCore" |
| `DBT-P75-056` | `2bcf3db`, `0730ecf` | SCM host: log failure reported after the handler registers; rotating log; STOP_PENDING; RUNNING after composition; exit when `register()` fails; installer probe gates `sc stop`'s reported state | first Windows compile and tests: CI `36275749129` (workflow_dispatch at `0730ecf`) green, no warnings in the maintenance-service build; runner behaviour: `windows-installer.yml` run on the PR head (see PR) |

Not run locally: Windows-target clippy (the service graph pulls `libsqlite3-sys`).
