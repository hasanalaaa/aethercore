# P80 report

P80 makes the hardware verdict say what was measured, gives the diagnostics snapshot a typed home for
thermal, battery, boot and network readings (owner decision D6), and stops a finding from being
opened or closed by a reading that is not a current reading of the same thing. Four tasks on one
branch (`lane2/p80`), from `main` at `e685b14`. A statement without the command that produced it is
labelled a belief. The full-CI run and the merge sha of this phase are not in this file (it lands
with the phase); they are in the PR and in the planning session's status.

## 1. Tasks

| task | commit | what changed |
|---|---|---|
| P80-01 | `a812146` | `hardwareVerdict` and the Hardware card; the Deep Scan's clear headline is "No issues in the measured checks" |
| P80-02A | `5cb8aa0` | `diagnostics.proto` additions, `hardware-telemetry::measurements`, four snapshot fields, NVMe spare threshold |
| P80-02B | `fd55310` | service mapping, UI rows (`measurement-rows.ts`, `MeasurementRows.svelte`), fixture with 32 zones |
| P80-03 | `d9eaeaa` | `ResourceRef.identity`, storage/memory freshness, clock-step tolerance, lifecycle guard |

## 2. Fast checks (no GitHub CI)

Mac, in `~/dev/aethercore-lane2`:
`node --experimental-strip-types --import ./apps/ui/tests/resolve-ts.mjs --test <the seven files of ci.yml>` 57 pass;
`svelte-check --threshold warning --fail-on-warnings` 0 errors, 0 warnings;
leak gate against the fixture: clean 9/9, injected 5/5, `--plant text|attr|arg` each detected;
layout sweep (`deepScan,repair,cleanup,startup,hardware`, widths 1280/960/720, en+ar, dark+light) 60/60;
`cargo clippy -p aethercore-pc-intelligence -p aethercore-hardware-telemetry -p aethercore-diagnostic-engine -p aethercore-maintenance-service --all-targets --locked -- -D warnings` clean;
`python3 scripts/static_validate.py` no failures; `python3 scripts/source_seal.py` OK after every task.

PC (`aether-win`, worktree `C:\dev\lanes\l2`, `CARGO_BUILD_JOBS=8`, own `CARGO_TARGET_DIR`), at `d9eaeaa`:
`cargo fmt --check` exit 0;
`cargo test -p aethercore-hardware-telemetry -p aethercore-diagnostic-engine -p aethercore-crash-diagnostics -p aethercore-contracts -p aethercore-pc-intelligence -p aethercore-maintenance-service --locked` exit 0, no failures (this compiles the Windows-only providers);
`pnpm --dir apps/ui check` 0 errors 0 warnings; the seven UI unit-test files 57 pass; `static_validate.py` no failures; `source_seal.py` OK.
The leak gate and layout sweep were run on the Mac only (they need Chrome and run in `ui-leak-gate`
on macOS in CI).

## 3. Red before, per task

| task | test | failed because |
|---|---|---|
| P80-01 | `deep scan headline …` (renamed key), `hardware verdict: not-collected, unavailable, partial, action and clear are distinct` | `deepScan.status.noneFound` did not exist; `hardwareVerdict is not a function` |
| P80-02A | `crates/diagnostic-engine/tests/measurements.rs` (6 tests) | the module and the snapshot fields did not exist (compile error: a compile-red, not a behaviour-red) |
| P80-02B | `measurement_presence_survives_the_service_boundary`; `measurement rows keep "not read" apart from zero …` | with the mapping disabled: `index out of bounds: the len is 0 but the index is 0`; the rows module did not exist |
| P80-03 | four `lifecycle::tests` with the API stubbed but no behaviour | another disk in the slot resolved the old finding; a stale healthy reading proved health; a stale reading raised a finding; a record stamped after "now" by a clock that moved back was dropped |

`a_disk_that_stops_answering_leaves_its_finding_unresolved_and_unverified` passed before the change:
the behaviour existed, this is a missing regression test.

## 4. Decisions applied

D0/D6 (additive wire, typed domains, no bag, no plugin framework). D7 (vendor sensors) not built:
`DBT-P80-004`, closed as deferred. D9 and D12 do not touch P80. H1 (no network at rest) unchanged:
no provider was added, and the lists are empty until P81–P83.

## 5. What stayed unmeasured or open

- No producer fills thermal, battery, boot or network yet; the page shows nothing for an empty list
  rather than "measured" (P81–P83).
- `DBT-P80-005`: the Deep Scan `FindingCard` has no per-finding coverage line or next step.
- The Hardware card was not seen on the owner's Windows 11 with Narrator, at 200% zoom, in Arabic.
- The freshness windows (storage 24 h, memory 5 min) and the one-hour clock tolerance are first bounds,
  not measurements from a machine.
- Two deviations from the task lists, both to avoid a dependency change: `maintenance-service` and
  `pc-intelligence` do not depend on `hardware-telemetry`, so the service reaches the types through a
  re-export in `diagnostic-engine`; and `crash-diagnostics` was not changed (boot records live in
  `measurements`; per D10 no coupling was added).
- P80-02B is ~360 changed lines including catalogs, fixture and tests (the cap is ~300 non-generated).


## P80-01 acceptance follow-up — DBT-P80-005 (2026-10-02)

The card now states the number and dates of its cited observations, missing observation time,
limits of that evidence and a single next step. Hardware-error evidence explicitly does not identify
an individual failed component, DIMM or driver. Other findings explicitly limit their scope to the
cited observations, without claiming an exhaustive resource test or an established underlying cause.
The existing owned uncertainty key remains visible. The next step comes only from an exact matching
remediation candidate with an owned translation; missing/unknown candidates require evidence review.
An unavailable or missing recheck instead asks for a fresh local scan before acting. No mutation is
started by the card. EN/AR use the existing catalogs, original wire facts and remediation candidates;
there are no wire, dependency or deadline changes.

`citedOnly` decorates each finding with an evidence chip under the same property name as its typed
facts. DeepScanPage now preserves those original facts before decoration and restores them at the
card boundary. A real populated-browser check caught this integration issue before completion.

Validation: four real Svelte server-render tests failed before implementation (EN/AR measured scope,
limits and next action absent), then passed; the complete UI test suite passed 82/82. Svelte check
reported zero errors/warnings; the production build passed (existing chunk-size advisory unchanged).
Static validation passed 351 checks. On this checkout's isolated fixture server at port 1483, eight
Deep Scan layout cases (1280/640 px, EN/AR, dark/light) had zero overflow, clipping or control overlap.
The Arabic rendered-DOM/font gate passed 7/7; clean/injected leak gates passed, and the planted text
negative control was detected (one sentinel). Rendered gates follow the real Overview scan button
because Deep Scan is no longer a rail item; gate assertions are unchanged. The gate route change is
separate commit `1032850` and is sealed together with this follow-up.

These fixture checks prove transformation and browser rendering. 640 px is a narrow-layout check,
not evidence of native Windows 200% zoom or Narrator acceptance; that installed qualification remains
NOT RUN for this follow-up. Hardware coverage matrices remain explicit separate qualification gaps.
