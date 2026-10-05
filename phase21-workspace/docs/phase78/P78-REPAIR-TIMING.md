# P78-04 assessment timing follow-up

The assessment page previously showed its start time and current check, but omitted elapsed and last-update information required by ASTRA-PLAN.md lines 109–114. The elapsed/wire omission in P78-REPORT.md section 6 is resolved for the assessment path without adding a protocol field or a service query.

## Actual source path

`operation-kernel/src/event_bus.rs:255` stamps an ordered EventEnvelope with its service emission time. `contracts/proto/events.proto:102` already carries it; the desktop's camelCase UiKernelEvent (`apps/desktop/src/main.rs:45,199`) preserves it. The existing repair watcher (`services/maintenance-service/src/streaming.rs:320`) still publishes assessment changes only. `apps/ui/src/platform/stream-state.ts:330` now retains the timestamp of the accepted assessment event, separately from its payload. It is a **last service observation**, not the time of the last progress change.

`apps/ui/src/features/repair/RepairPage.svelte:23` binds timing to session reset generation, assessment UUID and observation timestamp. Its local clock advances only while that page is mounted, connected and Scanning. `elapsed.ts:15` computes monotonic elapsed time rather than counting frames, so hidden-tab frame suspension does not lose active time; stopping cancels future publication. Offline and terminal views freeze; session/new UUID/new observation reset the clock. RPC responses carry no envelope timestamp, so a different UUID clears the observation rather than inheriting an old one (`controller.ts:22,33`). The existing terminal-over-late-Scanning settlement remains intact.

The displayed **Observed elapsed / المدة المرصودة** starts from the confirmed service observation minus the actual assessment start. Reopening without a new event deliberately uses that conservative observed baseline, rather than inventing wall-clock time while unobserved. Terminal duration uses the actual completion timestamp. Missing, reversed, unsafe or future service epochs produce unavailable time. Offline text explicitly pauses the display without claiming the underlying assessment stopped. Last service update remains dated service evidence. Seconds stay outside the phase live region with aria-live off; elapsed time never becomes a completion percentage. Both catalogs provide the same labels.

## Verification on the isolated source

- Initial actual reducer/RepairPage SSR controls: **5 failed before implementation**, including missing observation state, timing region and EN/AR timing labels. Raw receipt `aethercore-p78-timing-red.log`, SHA-256 `8317e62fa9d856ddbc594ddc9bd21e55215fdcf354839dd432765f2494e9844a`.
- Offline display qualifier: **2 failed / 6 passed before its implementation**, then passed. Raw receipt `aethercore-p78-offline-red.log`, SHA-256 `27be9784f9b89ad4b4a1d74e3269dd4d5c39b4b712b8fe99dbc5746961e77b6e`.
- `node --experimental-strip-types --import ./apps/ui/tests/resolve-ts.mjs --test apps/ui/tests/*.test.ts`: **115 passed**, including 8 timing controls exercising the real reducer, RPC controller, Svelte SSR in EN/AR, monotonic frame stop/resume/background behavior and invalid timestamps. Raw receipt `aethercore-p78-ui-full.log`, SHA-256 `1fac5a1596f4f020dcd3c2e471551da8d47fac65c3e9dd75146503c6c6ed1970`. Existing unrelated fixture tests emit localStorage/translation negative-control warnings.
- `pnpm --dir apps/ui check`: **0 errors / 0 warnings**.
- `pnpm --dir apps/ui build`: passed; existing large-chunk warning remains, without changing its threshold.
- `python3 scripts/test-phase12-localization.py`: **34/34**.
- `python3 scripts/zenith-adversarial-audit.py`: **35/35**, including no interval polling.
- `python3 scripts/enterprise-adversarial-audit.py`: **88/88**.
- `python3 scripts/static_validate.py`: **351 checks, failed=[]**; YAML parsing is explicitly **unmeasured** because PyYAML is absent in this Mac environment. No gate was weakened to hide that limitation.
- `git diff --check`: passed. Source seal is regenerated from the explicit delivered file set and verified separately before commit.

These are source/portable controls. No Windows servicing operation, installed application mutation, native Narrator/RTL/200% display qualification or new native assessment was executed. No Rust, wire or dependency changes are included. Full phase/native CI belongs to the integrator; prior CI is not attributed to this source.

Coordinator review added an actual controller regression for delayed start/cancel replies across
new streamed assessment identities and reset sessions. It first failed by replacing the new UUID
with the old response. Both handlers now share the request-generation/identity admission check;
a cancellation reply also must name its requested assessment. A rejected reply preserves the new
observation clock. The nine actual reducer/controller/render/clock controls and Svelte check pass.
External receipts: `/tmp/aethercore-p78-late-rpc-{red,green,check}.log`.
