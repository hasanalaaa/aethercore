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
measurement, not a Windows wall-time claim.

Windows CI `36180691145` at `ca16e44` (windows-2025, 2 vCPU, CPU-only;
llama.cpp is always built `Release`, `LLAMA_LIB_PROFILE` default) measured:

* assistant: `24 token(s) in 5985 ms (deadline 20000 ms)`;
* insight: `deadline exceeded after 640 of 927 prompt token(s)` — the runner
  cannot even prefill the insight prompt inside the 10 s budget.

The deadline was not widened. The test that asserted a model-served insight on
every host was asserting hardware, not code; it is now
`the_real_model_insight_path_is_cited_and_truthfully_badged`. On every host the
answer must be non-empty, fully cited and badged with the engine that served
it (`ruleFallback` when the model misses its deadline); a model-served answer
must arrive inside 10 s. macOS, where the model measured 3 insights in 2583 ms,
must be served by the model. **So `DBT-P56-002` closes for the code path, and
the Windows CPU budget is `DBT-P62-004`'s, still OPEN: on the 2-vCPU runner every
insight request spends 10 s in the model and then returns rule findings.** A
shorter insight prompt is the next lever; that is not done here.

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
