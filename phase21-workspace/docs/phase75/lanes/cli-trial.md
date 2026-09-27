# P75 lane `cli-trial` (part 3, trial run) — evidence

Found by driving every aetherctl verb against the unix maintenance service on this Mac
(`aethercore-maintenance-service --foreground --data-dir <scratch>`, unix-ipc build).

| row | commit | exposed by | what was wrong | now | red-before |
|---|---|---|---|---|---|
| `DBT-P75-063` | `ec0da48` | `aetherctl doctor` on a fresh service | exit 5, `rejected by service (diagnostics.stateUnavailable)`, until another command had scanned | doctor collects (read-only scan) and reports within `--timeout-ms` | `doctor_on_a_fresh_service_collects_and_reports`: `left: 5, right: 0` |
| `DBT-P75-064` | `f977660` | `cargo test -p aetherctl -p aethercore-maintenance-service --features aethercore-maintenance-service/unix-ipc,aetherctl/e2e --test phase28_cli_matrix` on `origin/main` | 6 passed, 1 failed (`self-check exited 0: left: 8`): the model is verified beside the binary; the suite has no CI caller | the test lays the model out beside the binary | 7/7 now |
| `DBT-P75-065` | `32d7da2` | `aetherctl --lang ar doctor` | every label English (7 of 114 printed fields had a catalog label) | Arabic label for every printed field; English text unchanged | `every_printed_field_has_an_arabic_label_and_english_is_unchanged`: 107 fields listed |

Local proof: `cargo test -p aetherctl --bins` 36/36; the e2e suite 7/7; `cargo clippy -p aetherctl --all-targets -D warnings` clean; `phase28-adversarial-audit.py`: its own checks PASS, exit 1 from the nested `p27-wirefreeze` checks (identical on `origin/main`, `DBT-P75-077`).
