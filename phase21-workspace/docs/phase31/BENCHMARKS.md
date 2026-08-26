# Phase 31 — Benchmarks (criterion)

Machine: macOS Apple Silicon, rustc 1.97.1, criterion 0.5 (release profile)
Recorded: Phase 31 (W8). Benches compile in CI via `cargo bench --no-run`; execution
was run ONCE here to record the baseline. NOT_EXECUTED in CI runners.

| Benchmark | Criterion estimate |
|---|---|
| `perf_ring_push_aggregate_60` (60-sample ring aggregate) | **1.86 µs** [1.847–1.877] |
| `export_verify_10k_records` (EXPORT_V1 chain verify, 10k) | **31.54 ms** [31.29–31.81] |
| `sqlite_diagnose_healthy_10mb` (read-only PRAGMA pass) | **4.30 ms** [4.22–4.39] |
| `timeline_page_read_2000_events` (plan_events page read) | **493.9 µs** [489.1–499.6] |

Notes:
- Numbers are machine-stamped baselines only — no regression gate is enforced yet.
- The sqlite bench builds a fresh ~10MB WAL db per run (fixture cost excluded from
  measurement by construction: build happens outside the iterated closure).
