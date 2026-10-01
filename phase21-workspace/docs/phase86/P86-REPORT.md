# P86 acceptance report

Implementation source head: `042f3b51970232f827b3313c60895c5166890f12`.
Shared model/core native head: `2f01d4c634db2e471e633f7a127a02d4c0e3db04`; repair priority native fixture head: `62a2888b813763b699f5f977b4d63ce90148d74b`.
The last source commit changes only a fake backend test summary key to the owned observation key.
Owner authority: `docs/roadmap/DECISIONS.md`, D22 and D23, approved 2026-09-28.
Scope: P86-02A/B, 03A/B, 04, and 05 following the existing P86-01 baseline.
Code and fixtures are complete; hardware/UI qualifications below remain explicitly NOT RUN.

## Behavior and acceptance

- Both assistant and insight model paths request the same strict, bounded fact-ID JSON schema.
  Unknown/duplicate IDs, extra prose/fields, and invalid selections cannot supply a factual meaning.
  The final meanings, states, domains and numbers come from typed propositions and owned EN/AR templates.
  The insight selector additionally rebuilds only exact owned single-fact candidates with their matching citation.
  A valid citation beside a completed-to-cancelled inversion failed the new regression before the fix.
- Invalid selections use the same typed facts and an explicit `ruleFallback` badge. Untyped insight packs
  prove only evidence availability/count and never success, health, verification or recurrence.
  Known assistant topics use a synchronous typed fallback while a model is absent/loading; unknown
  topics retain their declared fault/loading response. Mutation, cancellation and single-flight guards remain.
- The shared composer reads owner-scoped diagnostics, repair assessments and bounded Care history.
  Current diagnostics/repair facts expire after 15 minutes; future, failed and stale observations cannot
  become present-tense healthy facts. Critical diagnostics precede warnings and old maintenance history.
  Repair corruption/failure precede lower-priority issues before the eight-fact cap; healthy rows cannot
  consume that cap (new red-before regression). Care stopping for consent or finishing with incomplete/failed steps cannot be rendered as success.
  No request starts a new diagnostic scan, repair or Care run.
- A contextual diagnostic entry is visible only for an exact evidence ID/surface in the current pack.
  It opens an editable suggested question; it never sends it. Language changes clear/cancel old turns,
  and late RPC/stream replies cannot recreate the old transcript.
- Insight cache entries are scoped by owner, locale and evidence digest. The composer is read again after
  inference; a fake backend that changes a fact while answering returns/caches nothing. The renderer
  rejects late replies after language/evidence replacement and does not display locale-less insight events.
  ListInsightsRequest adds only optional `locale = 1` under D23; omitted/empty remains English, unsupported
  values are refused. Protocol version and existing tags remain unchanged.

## Local verification

| Check | Result |
| --- | --- |
| Core unit suite | 30 passed |
| Adversarial suite, including cited semantic inversion and strict insight selection | 29 passed |
| Full service binary unit suite, including 12 intelligence fixtures and repair priority | 43 passed |
| Real model shared insight path, including AR and cross-path single-flight | 3 passed |
| Real Unix IPC, optional list locale omitted/empty/EN/AR and invalid FR | 1 passed |
| UI pure suite | 61 passed |
| Svelte check | 0 errors, 0 warnings |
| UI build | Passed; existing size advisory retained |
| Desktop and aetherctl cargo check | Passed |
| Contract binary compatibility | 1 passed, old empty messages decode |
| Clippy, core all targets and service test/binary targets | Passed with warnings denied |
| Static validation / contract audit | 349 / 351 passed |
| Delivered source seal at implementation code head | 1571/1571 plus .github 7/7; report delivery adds five tracked artifacts |

The previous assistant-only full embedded suite also passed 11 tests, plus 30 core unit,
26 adversarial and one offline-boundary test. After the shared path changed, the affected real
insight tests and full core/adversarial suites were rerun rather than claiming the old receipt covered it.
No dependencies, artifact, RAM/token caps, 10-second insight ceiling or 20-second assistant ceiling changed.

## Windows native measurement

Raw structured receipts: `P86-WINDOWS-BASELINE.json` and `P86-WINDOWS-SHARED.json`.
Commands use one targeted cargo job, `CARGO_BUILD_JOBS=2`, an isolated lane checkout/target, and the
existing shipped artifact. No live cleanup, repair, driver installation, scan or model download ran.

Host: Windows 11 Pro 10.0.26200, Intel Core i7-14700K, 28 logical processors,
34,022,739,968 bytes physical RAM (approximately 31.69 GiB), Rust/Cargo 1.97.1, debug Rust test profile.
Pinned model: Qwen2.5-1.5B-Instruct Q4_K_M,
SHA256 `6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e`.

Baseline source `5bbe3fe894dbf53123142141bc4e02fff8412193`:

| Sample | n | p50 | p95 |
| --- | ---: | ---: | ---: |
| EN warm fact selection | 10 | 960 ms | 1043 ms |
| AR warm fact selection | 10 | 925 ms | 1021 ms |

Cold application verification: 34,205 ms; model load: 1,665 ms. These are separate phases,
not added to a claim about warm generation. Filesystem cache was not flushed. Validation took
26–66 microseconds. Schema accepted 20/20, rejected 0/20; pre-cancelled generation produced zero tokens.
Core unit 30, adversarial 27 and offline dependency-boundary 1 passed natively on this baseline.

Shared model source `2f01d4c634db2e471e633f7a127a02d4c0e3db04` (detached exact head, no tracked-source overlay):

| Sample | n | p50 | p95 |
| --- | ---: | ---: | ---: |
| EN warm fact selection | 10 | 944 ms | 1092 ms |
| AR warm fact selection | 10 | 921 ms | 977 ms |

There are **20 total warm samples, 10 EN plus 10 AR**, not 20 per locale. Schema accepted 20/20,
rejected 0/20; pre-cancelled generation produced zero tokens. Cold application verification was
33,827 ms and model loading 1,817 ms. The additional shared insight selector calls returned one
exact owned, cited fact as `localModel` in each locale: EN 912 ms, AR 918 ms. They are separate
from the 20 assistant fact selections. Native core unit 30, adversarial 29 and offline boundary 1 passed.
Native build output is an untracked isolated target outside the sealed workspace; it is not a source overlay.
The later service-only repair prioritization delta passed all 12 native intelligence fixtures at `62a2888`
(detached exact checkout, no source overlay); the shared core
has not changed since the measurement. Final integration checks must use the coordinator's exact head.

Two self-hosted CI jobs were active when measurement was scheduled. These timings therefore include
possible concurrent load and do not establish isolated latency, UI round-trip latency, or production
release performance. Ten samples per locale over two supported questions do not establish general
model quality or a statistical schema-rejection rate. Cold hashing dominated this debug-profile run;
no hash verification was skipped or cached to hide it. Serial measurements have no queued model work;
queue latency under real contention was not measured.

## Explicit remaining qualifications

- Low-memory and middle-class Windows cold/warm matrix: NOT RUN; only the high-end host above is available.
- New contextual entry with native keyboard/focus/Narrator, both locales and widths: NOT RUN on a real install;
  retain this for P87 acceptance. Static, layout compilation and late-reply regressions are not Narrator evidence.
- Live in-flight cancellation latency and an OS socket trace spanning generation: NOT RUN. Pre-cancel/native
  and the offline dependency-boundary checks are reported separately; they are not a full network trace.
- Final integration workspace CI/installer receipts belong to the coordinator and must use its exact final head.

## Reproduction

```text
cargo test --locked -p aethercore-intelligence-core --test embedded_generation windows_fact_acceptance_measurement -- --ignored --nocapture --test-threads=1
cargo test --locked -p aethercore-intelligence-core --lib --test adversarial --test offline_boundary -- --test-threads=1
cargo test --locked -p aethercore-maintenance-service --bin aethercore-maintenance-service intelligence::tests
cargo test --locked -p aethercore-maintenance-service --features unix-ipc --test phase27_unix_ipc unix_socket_round_trip_capabilities_ping_engine_source
```

The new long native sampling test is explicitly invoked acceptance work, not an added slow requirement
for every unit CI. Existing embedded tests remain enabled and their deadlines are unchanged.
