# Sigma Master Source Closure

Sigma Master is the final broad source-transformation layer before Windows production qualification. It does not redefine native evidence as source evidence.

## Source-closure commands

Write all evidence outside the repository:

```text
python scripts/regenerate-source-manifest.py
python scripts/sigma-master-source-closure.py --output-dir C:\AetherCoreEvidence\sigma-master
```

The source-closure orchestrator executes the Omega/Sigma source qualification twice, the five-case evidence-integrity regression, platform-neutral UI overlap-state tests, the actual-application E2E source contract, fuzz and coverage status gates, named-pipe qualification status, and the unsafe-boundary inventory. Expected unavailable toolchains are emitted as typed blockers rather than converted to PASS.

`SOURCE_CLOSED` means the executable source-level gates supported on the current host passed and repeated qualification preserved identical source fingerprints. It does **not** imply `BUILD_VERIFIED`, `NATIVE_QUALIFIED`, `SIGNED`, `RELEASE_CANDIDATE`, or `GA_APPROVED`.

## Full-application UI boundary

`scripts/sigma-master-full-app-ui.py` targets the real `App -> AppShell -> feature` component tree. The deterministic test transport is accepted only when the UI is built with `VITE_AETHERCORE_TEST_TRANSPORT=1`; normal production builds resolve directly to the Tauri transport. If the pinned pnpm graph is unavailable, the suite returns `BLOCKED` instead of falling back to a synthetic page.

The historical 37-check Chromium interaction suite remains motion-primitive evidence only.

## Final Windows qualification

`scripts/sigma-master-windows-qualification.ps1` composes locked fmt/check/test/Clippy, frozen UI build/check, evidence-integrity, full-app UI, inherited Phase 16 gates, pipe security, resilience, named-pipe teardown evidence, stress/soak, installer lifecycle, and required host witness coverage. The machine-readable acceptance plan is `release/sigma-master-windows-qualification.json`.

The named-pipe checker intentionally remains fail-closed while synchronous `CancelSynchronousIo` transport is retained: missing or out-of-bounds native teardown evidence blocks qualification and requires overlapped/event-driven migration if the bounds cannot be proven.
