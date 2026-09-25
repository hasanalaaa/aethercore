# P75 lane `insight-model` — evidence

Branch `lane/insight-model`. Scope: `crates/intelligence-core/**` and the
intelligence/assistant model block of `services/maintenance-service`. No
deadline, dependency or lockfile was changed.

## `DBT-P56-002` — real model insights

The embedded model previously served assistant turns but the insight path
returned a pending-generation error and always used the deterministic rule
engine. `LlamaCppReasoner::infer` now decodes within its existing 10-second
insight budget, parses bounded insight records, and drops every candidate
whose citation is absent from the current typed evidence pack. The selector
falls back after citation validation, so an all-uncitable model response does
not become an empty, falsely successful local-model result. The assistant
and insight paths clone one loaded model and share one generation gate; a busy
assistant refuses `Busy` while a busy insight falls back immediately.

The service's original second model load was caught by `cargo check --workspace
--locked`: the removed `load_streaming_reasoner` call failed to compile. The
composition block now receives a clone from the one verified activation.
`cargo check --workspace --locked` then exited 0. `ASSISTANT_DEADLINE`
remains 20 seconds.

## Evidence labels and budget (`DBT-P62-004`)

The service now places only detected recurrence patterns in the
`TimelinePattern` evidence surface. It previously labelled raw ingested
events as patterns. Model tests run over the actual pinned GGUF artifact;
they check that every returned citation resolves, the engine is `LocalModel`,
and the model remains inside its deadline. On this Mac, the selector test
printed `3 insight(s) in 2583 ms (budget 10000 ms)`. That is a Mac
measurement, not a Windows wall-time claim. Windows CI `36180691145` at
`ca16e44` is running; its real-model timings must be read from its log.

## Local verification

* `cargo fmt --all -- --check`: pass.
* `cargo clippy -p aethercore-intelligence-core --all-targets --locked --no-deps -- -D warnings`: pass.
* `cargo test -p aethercore-intelligence-core --locked`: pass, including real-model tests (the real-model group: 7 passed in 263.48 s).
* `cargo test -p aethercore-maintenance-service --locked`: 21 unit and 1 integration test passed.
* `cargo check --workspace --locked`: pass after the shared-load fix.
* `static_validate.py`: 347 checks, none failed; `test_gate_readers.py`: all 14 fail closed; `ps_marker_scan.py`: 234 assertions, 0 failed, 3 unmeasured.
* `source_seal.py`: 1497/1497 tracked files verified.

Full workspace tests, current-main integration, Windows CI at the final PR
head, and main push CI at its merge SHA remain required before closing either
ledger row.
