# Synthetic Driver Lab

Deterministic fixtures live in `tests/fixtures/phase18/scenarios.json` and cover D18-01 through D18-16: healthy/current, missing driver, OEM-vs-generic, self-built GPU, NVIDIA utility, multiple official candidates, unsigned, unexpected publisher, hostile redirect, candidate substitution after consent, offline, partial provider failure, firmware, ignore exact version, rollback evidence, and reboot aggregation.

`authority-ranking-matrix.json` covers OEM/self-built × exact/compatible × OEM/WUA/component authority plus negative invariants for unsigned, firmware, manual-official, and partial coverage. Rust unit tests additionally exercise version parsing, exact-ignore staleness, firmware exclusion, trust ranking, OEM/self-built context, provider coverage, redirect policy, sealed plan invalidation, and batch containment.
