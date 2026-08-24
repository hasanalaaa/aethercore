# AetherCore Phase 20 — Master Delivery Report

**Closure claim:** `PHASE_20_PERFORMANCE_INTELLIGENCE_SOURCE_COMPLETE`
**Baseline input:** AetherCore-Phase19-Source.zip (SHA-256 verified against FINAL_DELIVERY_HASHES.json; 593 files extracted, cross-checked against BINARY_SAFE_RECONSTRUCTION_PROOF.json)
**Delivery tree:** this directory = sealed Phase 19 source + baseline repair + Phase 20.

---

## 1. What shipped

### Baseline repair (precondition — Phase 19 did not compile)
~40 mechanical defects across 10 crates repaired to a green baseline of **298 tests / workspace check clean**. Full inventory and classification in `docs/phase20/ARCHITECTURE.md` §"Baseline repair disclosure". Every repair is byte-attributable in the reconstruction patch.

### Domain A — performance-telemetry crate
Passive, owner-scoped, bounded sampling pipeline. PDH + CallNtPowerInformation on Windows with RAII-wrapped handles and per-collector fault isolation; deterministic synthetic platform for audits. Ring capacity 300 samples, interval floor 250 ms (observer-effect guard), hostile-input normalization at every ingress. **9 adversarial tests.**

### Domain B — performance-bottleneck crate
Eight deterministic rules producing role-classified findings (root cause vs contributing condition vs symptom) with measured evidence citations and materialized causal edges. Refuses speculation below 5 samples; CONFIRMED requires ≥10. Byte-deterministic reports including digest. **11 attribution tests.**

### Domain C — performance-optimization crate
Reversible-only action surface (EcoQoS, background priority, cooperative trim, Game Mode) behind consent policy fixed by kind; machine mutations through the shared MutationSupervisor (`MutationWorkload::Optimization` added) with CommitFence publication; journaled restorable changes; audit platform proves no destructive verb can enter the operation vocabulary. **9 governance tests.**

### Wire contract v7 (additive)
`performance.proto` (requests/responses/events), EventKind 22–24, envelope payloads 31–33, response tag 44. Frozen tags 0–21 unchanged — asserted by the adversarial audit.

### Service integration
`PerformanceEngine` in `ServiceContext`, constructed over the kernel mutation supervisor; principal-scoped router handlers for start/stop/snapshot/analysis/planning; execution deliberately gated behind the desktop consent handshake (returns Forbidden without it).

### Desktop + renderer
Five Tauri commands; typed contracts; stream-state slices (`performance`, `bottleneckReport`, `optimizationStatus`, `perfSampling`); event normalization for the three new payloads.

### Domain D — Performance page
Metric cards with live sparklines, throttle warning strip, finding cards with selection → review-plan tray, safety-contract note. Pointerdown feedback via shared fluidPress; springs only; reduced-motion/transparency honored via global preference store; **EN/AR parity: 38 new keys each side, verified programmatically**, Arabic plurals for finding counts.

---

## 2. Verification matrix

| Gate | Command | Result |
|---|---|---|
| Workspace compile | `cargo check --workspace --all-targets` | EXIT=0 |
| Rust regression | `cargo test --workspace --jobs 2` | **327 passed / 0 failed** |
| Renderer types | `npx svelte-check --tsconfig ./tsconfig.json` | 0 errors (17 pre-existing warnings preserved) |
| Adversarial source audit | `python3 scripts/phase20-adversarial-audit.py` | PASS (43 checks) |
| i18n parity | embedded in audit | EN=AR=38 perf keys |
| Wire stability | embedded in audit | tags 0–21 frozen |

Known harness issue: default-parallelism full-suite runs can hit SQLite contention in one driver-install test (P20-ISS-001); passes isolated and at `--jobs 2`.

## 3. Qualification debt

See `docs/phase20/QUALIFICATION_DEBT.md`. Highest-risk honest item: **P20-QD-003** — the service ships with the audit-only NoopPlatform; live EcoQoS/priority application is fully plumbed but requires Windows-native qualification before enablement. This is the anti-snake-oil posture applied to ourselves: we do not claim behavior we could not execute here.

## 4. Reconstruction

`PHASE_20_BINARY_SAFE_PATCH/` contains:
* `changes.patch` — unified diff of every changed/new file relative to the sealed Phase 19 tree.
* `new-files/` — full content of files that do not exist in Phase 19.
* `MANIFEST.json` — SHA-256 of every file in the delivered tree.
* `apply_patch.sh` / `verify.sh` — reconstruct-and-check scripts.

A verifier with only the sealed Phase 19 zip plus this patch can reproduce this tree bit-for-bit and re-run every gate above.
