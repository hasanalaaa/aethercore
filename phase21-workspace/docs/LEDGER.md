# AetherCore — project status ledger

**This file is the single authoritative status of the project.** It supersedes
the `§NN PROGRESS TABLE` sections inside `docs/phase36/SESSION_CONTEXT.md`,
which is 14,512 lines and can no longer be read by a cold session. The history
stays where it is; nothing was rewritten.

## How a cold session resumes

1. Read this file. Nothing else is required first.
2. Find the first row in **§1 Open work** whose status is not `CLOSED`.
3. Continue from it. Each row names its evidence and which machine it needs.
4. Move the row in the **same commit** as the work that moved it.

If a row needs a machine or a person you do not have, skip it — §3 says which
rows those are and who unblocks them. Do not redo a `CLOSED` row: its evidence
column names a commit or a measurement you can re-run.

Last moved: P58 (2026-09-12).

---

## §1 Open work

Status values: `OPEN` (something remains, the row says what) · `ACCEPTED`
(measured, deliberately not fixed, escape routes disproved) · `CLOSED` (done,
evidence names where).

| id | what it is | status | evidence | machine |
|---|---|---|---|---|
| `DBT-P36-006` | Windows filesystem security findings rest on a POSIX mode-bit approximation, not ACL evidence | OPEN | verified P55: `crates/security-audit/src/filesystem.rs:36-44`, the comment still reads "Windows has no POSIX mode bits" | any Windows |
| `DBT-P36-007` | its retirement condition (a seal) has not occurred | OPEN by design | not code-checkable; not actionable until the seal | — |
| `DBT-P41-001` | the chain's VC++ redistributable **install** branch has never executed on any architecture — only the "already present" branch has | OPEN | P49 §49.7 measured detection and the x64 `Launch` condition; the install branch is still unrun | a Windows box **without** the VC++ redist |
| `DBT-P42-012` | check 5 of 6: the `vcomp140.dll` pinned fallback is the ARM64 VM's layout, so every other machine fell through to a throw even with a good copy installed | **CLOSED** | `f54cc50` — reproduced by CI run `34680665151` on a third machine (`VCToolsRedistDir` not set, pinned path absent), then fixed by a `vswhere` probe. Discovery widened; the 193,152 B / `55aba23c…` hash check **unchanged and still strict**. Measured locally: found at toolset 14.44.35112, hash equal | done |
| `DBT-P45-004` | ~2% of macOS CPU samples degrade honestly instead of reading | ACCEPTED | P45 §45.3, 1/50; the bounded retry was deliberate | macOS |
| `DBT-P47-003` | `perProcessorBusyBp` is never populated on Windows | **ACCEPTED — P58 decision: deliberately out of scope, and the empty vector is the most honest of the three platforms** | P58 measured the consumers rather than the producer. **Nothing reads it.** Zero render sites in `apps/ui/src` (only the type in `contracts.ts` and the fixture in `layout-fixture.ts`), zero uses in `crates/performance-bottleneck`. The only readers are the wire encoder `services/maintenance-service/src/performance.rs:31` and `apps/aetherctl/src/offline.rs:308`'s JSON dump. **And no platform populates a real per-core breakdown**: `macos_impl.rs:224` and `linux_impl.rs:515` both write `vec![total_busy_bp]` — the aggregate, once. Wiring Windows alone would make it the only platform carrying a genuine per-core array, so the same field would mean two different things depending on the host — worse than empty. Cost, stated rather than implied: the counters would join the query `sample_cpu` already opens, so it is N extra `PdhAddEnglishCounter` + N reads per tick and **not** a second 100 ms sleep; the observer effect is small, which is why it is not the reason. The reason is that there is no caller. Retire this row by wiring all three platforms **and** a consumer, not by wiring Windows | any Windows |
| `DBT-P47-004` | GPU adapter identity and VRAM stay empty; no honest source wired | **SPLIT by P58. Identity: ACCEPTED, out of scope. VRAM total: BLOCKED-MACHINE** | The two halves have different answers because they have different consumers. **Identity** (`adapter_id`, `adapter_name`): nothing renders it — grep of `apps/ui/src` finds it only in `contracts.ts` and the fixture, and no catalog key names an adapter. Same argument as `DBT-P47-003`: no caller, so out of scope. **VRAM total** (`dedicated_total_bytes`): this one **does** have a consumer — `PerformancePage.svelte:227` renders `perf.vram` as used/total, and with total at 0 `bytesToGb` returns `—`, so the tile reads `5.0/— GB`. Honest, but a permanent hole in a shipped screen. Not wired this session because the brief's own condition — "prove the number traces to it" — **cannot be met from the Mac**; the Windows PC is unreachable (see §3). Recorded for whoever wires it: **`Win32_VideoController.AdapterRAM` is the wrong source.** It is a `uint32` and saturates at 4,293,918,720 B, so any adapter above 4 GiB reports ~4 GiB. The traceable sources are `HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\<NNNN>\HardwareInformation.qwMemorySize` (a `QWORD`) or DXGI `IDXGIAdapter::GetDesc().DedicatedVideoMemory`. WMI stays the right source for identity (`Name`, `PNPDeviceID`) | any Windows |
| `DBT-P49-002` | the consumer installer's UI | **3 of 4 FIXED and photographed; version NOT fixed** | `docs/phase55/P55-ITEM4C-LIFECYCLE.md` + `installer-screens/`. Title bar now "AetherCore Setup"; Æ mark renders; progress reads "Processing: AetherCore". **Version still absent — cause found: the shipped `thm.xml` has no version control at all, so `ShowVersion=1` has nothing to act on; fixing it needs a custom `ThemeFile`.** Licence still absent, deliberately | this machine |
| `DBT-P55-007` | **`signatureValidationMode=require` is not enforced on this developer machine.** A cold `dotnet tool restore` with deliberately corrupted `trustedSigners` fingerprints still succeeded (exit 0), so the supply-chain control the project believes it has does nothing here. The same restore is enforced on the CI runner, which is where NU3034 surfaced. Pre-existing, not introduced by P55 — but it means **no local build can verify this control**, and a developer-machine build cannot be treated as evidence that packaging dependencies were signature-checked | OPEN — security-relevant | verified P55 by negative control: fingerprints replaced with `0000…`/`1111…`, restore still exit 0 on SDK 8.0.424 | any dev machine |
| `DBT-P55-005` | `apps/desktop/icons/SOURCE.json` names `design/icon/aethercore-mark.svg` as the source of the shipped icon set | **CLOSED — the premise was a measurement error, and the pipeline reproduces the set byte-identically** | **The file is in the tree and always has been**: `design/icon/aethercore-mark.svg`, tracked on `main`, added in `25a6a5a`, sha256 `65f4417…36441f` — equal to `SOURCE.json`'s `sourceSha256` to the character. P55's probe was `Test-Path design/icon/aethercore-mark.svg` run with the working directory at `phase21-workspace`, and `phase21-workspace/design` does not exist; it measured the wrong path. `design/` sits at the repository root, which is exactly what `build-icons.mjs`'s `REPO = resolve(ROOT, '..')` resolves against. Regeneration verified P58: `node tools/icon-pipeline/build-icons.mjs --check` → **`PASS — every committed icon is what this source renders`, 17/17 byte-identical**, on Chrome 152.0.7977.84 (the set was rendered on 152.0.7977.76, so the pipeline is stable across that patch bump). All 17 committed hashes independently re-verified against `SOURCE.json`: 0 mismatches | any |
| `DBT-P49-003` | the bundle leaves an elevated log in `%TEMP%` after install and uninstall | OPEN — reproduced, pattern corrected — **BLOCKED-MACHINE (P58)** | verified P55 Item 4.C: three survivors, `%TEMP%AetherCore_20260912{125833,130229,130850}.elevated.log`, 920/929/929 B. **P49 recorded `AetherCore_Setup_*`; they are now `AetherCore_*`** because the bundle Name changed and Burn derives the log name from it — a sweep written against the old pattern reports zero and is wrong. P58 could not attempt it; see `DBT-P55-008` for the reachability evidence | this machine |
| `DBT-P49-004` | `intervalMs` does not describe the window the numbers were measured over | OPEN | verified P55: `windows_impl.rs:349` `let _ = interval;`, `:369` `from_millis(120).min(from_millis(100))` (the `120` is dead), `:710` hardcoded 80 ms, `lib.rs:330` publishes the *requested* value | any Windows |
| `DBT-P55-001` | `phase21-workspace/.github/workflows/{ci,fuzz,release}.yml` have **never been dispatched** | **CLOSED — cause confirmed by API, all three now registered and dispatching** | Cause measured, not inferred: before the fix `gh api repos/hasanalaaa/aethercore/actions/workflows` returned `total_count: 1` — only `.github/workflows/windows-installer.yml`. GitHub reads `.github/workflows/` at the repository **root** and nowhere else, so the three were never workflows, merely text. `8888d31` moved them and fixed them for the new location (`defaults.run.working-directory: phase21-workspace`, prefixed artifact paths). After: `total_count: 5` — ci `356644539`, fuzz `356644541`, release `356644542`, windows-installer `353735631`, plus Dependabot. `.github/dependabot.yml` had the identical defect and was moved in the same commit; its `directory` values were wrong too and were corrected. **What each run then found is `DBT-P58-001`, `DBT-P58-002` and `DBT-P58-003`** — the workflows dispatch; two of them do not pass | — |
| `DBT-P55-002` | `cargo fmt --all -- --check` failed across the workspace | **CLOSED** | `b4023ec`, a pure-formatting commit touching 128 `.rs` files and nothing else. Before: exit 1, **1,254 hunks across 128 distinct files**. After: exit 0, empty output. **The row's old figure was wrong in kind**: "1,247 files" counted `Diff in <path>:<line>:` lines, which are hunks. Proof the reformat is inert, by exit code on both sides: `cargo check --workspace --locked` exit 0 → exit 0, 85 warnings → 85 warnings, 0 errors → 0 errors | any |
| `DBT-P55-003` | `D:` cannot hold a second full system image beside the existing one: C: uses 571.6 GB, `D:` has 410.5 GB free | OPEN | verified P55 by enumeration, see §2 | this machine |
| `DBT-P55-004` | the recovery media is **not attached** — zero removable volumes carry `\sources\boot.wim` or `\bootmgr`. It is not merely un-boot-tested; it is absent, so it cannot be tested until it is found | OPEN | verified P55: `scripts/gate4-preconditions.ps1`, condition 3 | this machine |
| `DBT-P55-008` | the installer rendered **English only**, on a machine whose Windows UI language is Arabic | OPEN — **BLOCKED-MACHINE (P58)** | P55 Item 4.D, 80 captured frames, all English. P58 could not attempt it: the Windows PC is unreachable from this Mac — no `~/.ssh/config` exists at all, the only `known_hosts` entry is `[192.168.68.114]:8022` (absent from the ARP table, and port 8022 is not Windows OpenSSH), and no hostname, address or credential for the PC is recorded anywhere under `docs/`. P55's Windows work ran natively on that box (`P55-ITEM4C-LIFECYCLE.md:3`, "Machine: the physical Windows PC, elevated"), not remotely | this machine |
| `DBT-P55-009` | launched with `/uninstall`, the bundle still presents the "Modify Setup" chooser, and **Repair is the focused default** | OPEN — usability — **BLOCKED-MACHINE (P58)** | P55 Item 4.C: an unplanned repair ran to completion this way, `installer-screens/07-repair-complete-unplanned.png`. P58 could not attempt it; see `DBT-P55-008` for the reachability evidence | this machine |
| `DBT-P49-001` | `build-release.ps1` never set the ADK `DismApi\Lib\amd64` path into `LIB`, so a clean shell died `LNK1181` | **CLOSED** | `07445b4` — `crates/system-repair/build.rs` locates the library itself; the failure was reproduced first (exit 101, `LNK1181` ×3 with `LIB` unset), then measured at exit 0 | done |
| `DBT-P55-006` | two recorded ARM64 recipes still set `LIB` to the DismApi path by hand — `scripts/build-arm64-msi.cmd:77` and `scripts/p36vm/p36_relbuild.cmd:11`. Now redundant: `build.rs` resolves `arm64` the same way it resolves `amd64`. Harmless (build.rs treats existing `LIB` entries as a fallback), so they were **left alone rather than changed**: both are documented verbatim reproduction recipes for a VM this session cannot test on | OPEN — low | verified P55 by grep; retire them on the ARM64 VM with a build that proves the recipe still works without the line | the ARM64 VM |
| `DBT-P56-001` | commit `2be8396` is titled **"finish local AI chat"** and delivered no chat. The AI-facing wire verbs are `ListInsights`, `RequestInsight`, `DismissInsight` and nothing else | OPEN — recorded so the message is not trusted | verified P56: `grep -c chat services/maintenance-service/src/*.rs` → 0 on all 14 files | any |
| `DBT-P56-002` | **the embedded model had never generated a token.** `LlamaCppReasoner::infer_embedded` returned `Err` unconditionally, so every request degraded to `DeterministicFallbackReasoner` while `engine_label` still read `localModel` | **NARROWED, not closed.** The ASSISTANT path generates: `StreamingReasoner` is implemented over llama.cpp with a greedy sampler, a 1.15/256 repetition penalty, a 512-token ceiling, a 20 s deadline and a cancel flag checked between tokens. Measured on this machine: 82–138 tokens in 0.24–1.2 s, Metal, 151 MiB compute buffer; `tests/embedded_generation.rs` 5/5 green against the real artifact. **The INSIGHT path is unchanged** — `infer_embedded` still returns that same `Err` at `llama.rs:237` and insights still come from the rule engine | any |
| `DBT-P58-001` | **`ci.yml`'s first gate fails on a release blocker that has been committed since Phase 19, and the job cannot reach step 2.** `scripts/freeze-dependencies.ps1 -VerifyOnly` throws on `release/dependency-freeze.blocker.json` (`OMEGA-RB-001`, `severity: release_blocker`, `status: OPEN`), added in `b024df8`. Its stated reason is **still true**, measured: of the five files it names, `Cargo.lock` and `pnpm-lock.yaml` exist and `release/dependency-locks.sha256`, `release/dependency-manifests.sha256` and `release/dependency-freeze.json` do **not** — so `-VerifyOnly` would fail at `Assert-Lines` even with the blocker file gone. The freeze set has never been generated. `windows-installer.yml` goes green because it runs `-Refresh`, not `-VerifyOnly`; **no workflow in this repository has ever passed `-VerifyOnly`** | OPEN — **OWNER**, supply-chain risk acceptance | P58, CI runs `34707875531` and `34709219509`, job `windows`, step 1 of 16, exit 1 — identical on both, so the P58 gate-path repair introduced nothing. `deny-check` is green in both. The blocker's own closure text requires `-Refresh` on "the trusted dependency-freeze workstation" and review of the resulting graph. That is an approval of the current dependency graph, not a measurement, and `DBT-P55-007` records that a developer machine cannot verify signature enforcement — so generating the freeze set here would carry exactly the weakness the register already names. **Not removed, not skipped, not marked `continue-on-error`** | the trusted freeze workstation |
| `DBT-P58-002` | **`cargo fuzz` had never built a single target in this repository.** `fuzz/Cargo.toml` carried no `[workspace]` table and the parent workspace neither included nor excluded it, so cargo refused the manifest before compiling: "current package believes it's in a workspace when it's not" | **CLOSED** | `22e4a21`. Reproduced locally first — `cargo metadata --manifest-path fuzz/Cargo.toml` exit 101, the identical message — then fixed with the empty `[workspace]` table `cargo fuzz init` generates. **Two independent reasons this gate never said anything true**: it was never dispatched (`DBT-P55-001`), and `continue-on-error: true` on the fuzz step meant it would have reported success with every leg failing to build. Both removed. After: `cargo metadata` exit 0, 5 bin targets; `cargo check --workspace --locked` unchanged at exit 0 / 85 warnings / 0 errors. **Green end to end at run `34709222717`, 5/5 legs**, once `DBT-P58-006` was also fixed | any |
| `DBT-P58-003` | **`release.yml` is now a registered workflow that cannot run here.** Dispatched as run `34707902097` and it queued rather than started | ACCEPTED — this is the deferred certificate, not a defect in the file | P58 measured all three preconditions against the API: `actions/runners` → `total_count: 0`, so nothing matches `[self-hosted, windows, x64, aethercore-signing]`; `environments` → `total_count: 0`, so `production-signing` does not exist; `actions/variables` → `total_count: 0`, so `AETHERCORE_CODESIGN_THUMBPRINT` and `AETHERCORE_TIMESTAMP_URL` are unset and the "Verify signer configuration" step would throw even with a runner. **What would make it run**, in order: a production Authenticode certificate (§3 row 4), a patched self-hosted Windows runner registered with those four labels and the certificate non-exportable in its store, a `production-signing` environment, and the three repository variables | the signing runner |
| `DBT-P58-004` | **seven of the twelve fuzz target sources are not declared as `[[bin]]` and nothing builds them** — `ipc_frame`, `operation_state`, `pii_redaction`, `scheduler_eligibility`, `support_archive`, `update_manifest`, `windows_multisz`. `fuzz.yml`'s matrix lists five, which matches the manifest, so the gate is honest about what it runs and silent about what it does not | OPEN — low | verified P58 by diffing the `[[bin]]` names in `fuzz/Cargo.toml` against `ls fuzz/fuzz_targets/`. Left alone deliberately: wiring seven targets means seven crate dependencies and compiling code that has never been built once, which is its own phase rather than part of making the workflows dispatch | any |
| `DBT-P58-005` | **the audit scripts treat a missing file as an empty file, so a check asserting something is ABSENT passes vacuously against a file that was never opened.** Ten scripts read with the shape `p.read_text(...) if p.exists() else ''`. Moving the workflows in `8888d31` triggered exactly this: eight audits kept reporting normally while reading `""` for `ci.yml` and `release.yml`. `static_validate.py` was the honest one — it read directly and crashed, which is how the whole class was found | **OPEN — census published, repair next.** P59 ITEM 1.A enumerated the class across `scripts/`, `tools/` and `.github/workflows/`: **159 sites, 48 class B (10 live, 38 latent), 110 class A, 1 class C** | verified P58: `grep` finds the fail-open reader in `enterprise-adversarial-audit.py`, `phase13/14/15/16`, `phase33`, `phase34`, `zenith-adversarial-audit.py`, `zenith-recursive-audit.py`, `static_validate.py`. The P58 path breakage is repaired in `a8946bb` and each script's failure set was compared against `b4023ec` (newly failing: none, newly passing: none), **but the fail-open reader itself is untouched** — the next moved or renamed file does the same thing silently. The fix is for the readers to fail loudly on a path the script asserts about; that is a sweep across ten files and was not folded into a commit whose job was to repair a regression. **P59 measurement**: `docs/phase59/P59-READER-CENSUS.md`. Ten B sites are **live** — the gate's verdict is byte-identical with the source blinded, so it reports a pass on a file it never opens. Reproduced on the real tree without a harness: `mv scripts/phase13-fault-injection.ps1 /tmp/ && python3 scripts/phase16-ga-audit.py` → **exit 0, `{"ok": true, "checks": 42, "failed": []}`, byte-identical to baseline** — a GA gate deletes one of the five files it audits and still reports 42/42 green. The other live sites are in `phase15-security-audit` (6 sources), `enterprise-adversarial-audit:214` (84 UI sources behind one absence assertion), `phase13-reliability-audit`, `phase35-adversarial-audit` (4), `static_validate:585` and `:1393`, `phase30-adversarial-audit:222,238` (a ledger entry whose file is absent is skipped, not flagged) and `phase29-adversarial-audit:256` | any |
| `DBT-P58-006` | **`cargo fuzz` ran the pinned stable toolchain, not nightly.** `phase21-workspace/rust-toolchain.toml` pins `channel = "1.97.1"`, and a toolchain file outranks the installed default for every cargo and rustc invocation inside that directory — so `dtolnay/rust-toolchain@nightly` installed nightly and nothing used it. `-Zsanitizer=address` came back "the option `Z` is only accepted on the nightly compiler" | **CLOSED** | `a8946bb`. Reproduced locally: `rustc -Zunstable-options --version` inside the workspace gives the identical error. Fixed with `RUSTUP_TOOLCHAIN: nightly` on the step, which outranks the toolchain file and is inherited by the `cargo build` cargo-fuzz spawns. Only visible because `DBT-P58-002` was fixed first — cargo refused the manifest before it ever reached the compiler. Green at run `34709222717` | any |
| `DBT-P42-011` | x64 numeric bias in performance readings | **RECLASSIFIED — does not reproduce** | P49 §49.6, nine rounds on the original silicon; residual means smaller than the host counter's own ±14-pt spread | done |

### P55 items still in flight

| item | what | status |
|---|---|---|
| P55-1 | CI: green run of `windows-installer.yml`, run id recorded | **CLOSED — run `34683812129`, conclusion `success`, 58m0s, commit `d39e490`.** All 11 authored steps executed; **0 `continue-on-error`, 0 `if: always()`** in the workflow. Took four fixes: `07445b4` (ADK + DismApi), `f621ffc` (ADK log dir), `f54cc50` (vcomp140 via vswhere), `d39e490` (NuGet trusted signers) |
| P55-2 | this ledger | CLOSED — this file |
| P55-3 | recovery re-established | 3.A done (see §2). **3.B NOT DONE — owner decision 2026-09-12: do not re-image.** The existing image does not fit beside a new one, so writing one would destroy the only verified copy. `DBT-P55-003`, `DBT-P55-004` open |
| P55-4 | build + fix + install the consumer installer | **CLOSED.** 4.A+4.B in `INSTALLER-UI.md`, 4.C+4.D in `P55-ITEM4C-LIFECYCLE.md`. Sweep: **13 of 14 pass; check 14 fails with 3 named survivors**, all `DBT-P49-003` . **Machine end state: AetherCore 0.1.11 installed from the CI build** (separate restoration install, exit 0, binaries hash-match the artefact) |
| P55-5 | Gate 4 prepared to the owner line | CLOSED — `docs/phase55/GATE4-CANDIDATES.md`, `scripts/gate4-driver-runbook.ps1`, `scripts/gate4-preconditions.ps1`. Preconditions measured: **2 of 6 hold** |


### P56 items

| item | what | status |
|---|---|---|
| P56-1.A | measure the density before solving it — words of prose, type sizes, elevations, text-to-data, per screen | **CLOSED** — `apps/ui/tools/measure-density.mjs`, `docs/phase56/DENSITY.md`, raw in `docs/phase56/measure/` |
| P56-1.B | the design direction, written down | **CLOSED** — `docs/phase56/DIRECTION.md`, five principles with numeric targets |
| P56-1.C | rebuild the Overview to it | **CLOSED** — `docs/phase56/DENSITY-AFTER.md`. 15 type sizes → 6 (all tokens), 6 neutral surface levels → 3, prose 63 → 0 populated and 167 → 24 empty, words-per-reading 5.22 → 0.75 empty. Every gate green |
| P56-1.D | present it — screenshots, both languages, both themes, populated and empty. **Gate: no roll-out until the owner approves** | **CLOSED — APPROVED.** `docs/phase56/screens/{populated,empty}/overview-1280-{en,ar}-{dark,light}.png`, 8 images. The approval is recorded in `docs/phase56/P57-DRAWER-AND-ROLLOUT.md`: "The owner has approved the direction" |
| P56-2.A | the chat's interaction design | **CLOSED** — `DIRECTION.md` Part 2.A: an inline-end drawer, three named outcomes, the wire stated before implementation |
| P56-2.B | the wire contract | **CLOSED** — `crates/contracts/proto/assistant.proto`; request tags 92/93, response 92, `EVENT_KIND_ASSISTANT_TURN` = 30, envelope payload 39. Shape stated in `DIRECTION.md` before implementation |
| P56-2.C | the service side | **CLOSED** — `services/maintenance-service/src/assistant.rs`: worker thread, 120 ms stream throttle, per-principal cancel registry, boundary validation, and a fault path that never yields an empty answer. Router arms for both verbs; `AssistantCoordinator` in `ServiceContext` |
| P56-2.D | the UI | **CLOSED by P57 ITEM 1** — `apps/ui/src/features/assistant/`, the overlay drawer. See the P57 rows |
| P56-2.E | the tests that would catch the failure that matters | **RED, committed first** — 14 green in `assistant.rs` (ungrounded answer refused, fault not empty string, cancellation), 2 RED in `tests/embedded_generation.rs` against the real artifact. 2.C turns them green |
| P56-3 | roll the direction out to the remaining screens | **UNBLOCKED** — carried into P57 ITEM 3 |

### P57 items

Report: `docs/phase57/P57-REPORT.md`. Raw measurements:
`docs/phase56/measure/p57-{before,after}-{populated,empty}.json`.

| item | what | status |
|---|---|---|
| P57-1 | the assistant drawer | **CLOSED** — `apps/ui/src/features/assistant/{controller.ts,AssistantDrawer.svelte}`. 26rem overlay on the inline-end edge, full-width sheet below 58rem, `Ctrl+/` opens and focuses, `Enter` sends, `Escape` cancels then closes, available on every screen. Measured across 2 widths x 2 languages x 2 themes x 7 turn states: **56/56 clean**, 0 overflow, 0 clipping, 3-4 type sizes all tokens, 0 denied chips. `verify-arabic --page assistant` 7/7, **0 glyphs from a system fallback**. Contrast with the drawer open: 756 nodes, 0 below 4.5:1 |
| P57-2 | one numeral convention for technical readings | **CLOSED — Latin digits everywhere, both languages, the log included.** Reason and measurement in `DIRECTION.md` Part 3: JetBrains Mono has no Arabic-Indic digits, so `seq ١ ١٢:٣٠:٠٠` drew 7 glyphs monospaced and 7 from a proportional face inside one token. `formatNumber`/`formatDateTime` resolve to `ar-IQ-u-nu-latn`; units, date order and AM/PM stay Arabic. Side effect: `verify-numbers` now sees **34** score-shaped numbers, up from 27 — every Arabic percentage had been invisible to the gate, because `\\d` never matched `٥١` |
| P57-3 | the direction rolled out to the remaining eleven screens | **CLOSED** — `docs/phase57/P57-REPORT.md`, six commits, two screens each plus the two design-system steps (the type scale, the surface ladder). Twelve screens at 1280: prose **1,533 → 756** populated en and **1,136 → 162** empty en; words per reading **4.20 → 2.02** and **9.09 → 1.13**. Type sizes ≤6 on every screen, every one a token (was 3–13). Neutral surfaces ≤3 on every screen, populated AND empty (was 1–7). 71 catalog keys removed in both languages |

### P58 items

| item | what | status |
|---|---|---|
| P58-1 | `PageId` enumerated `'assistant'`, which is not a route | **CLOSED** — `apps/ui/src/lib/navigation.ts:4`. Removed from `PageId`; `IconName` keeps it, because the drawer does have an icon. Nothing depended on the page membership: no `NAVIGATION` entry, no `setPage('assistant')`, and `verify-arabic --page assistant` passes a CLI string, not a `PageId`. Measured after: `svelte-check` **229 files, 0 errors, 0 warnings**, `vite build` exit 0, and Ctrl+/ opens and focuses the drawer on **12/12** screens |
| P58-2.A | why the three workflows never dispatched | **CLOSED** — they were never workflows. `gh api .../actions/workflows` returned `total_count: 1` before, `5` after. `DBT-P55-001` |
| P58-2.B | `cargo fmt --all`, as its own commit | **CLOSED** — `b4023ec`. 1,254 hunks / 128 files → 0. `DBT-P55-002` |
| P58-2.C | make all three dispatch and go green | **PARTIAL, and the partial is the finding.** All three dispatch. **fuzz: GREEN — run `34709222717`, conclusion `success`, 5/5 legs**, after two real defects were found and fixed (`DBT-P58-002`, `DBT-P58-006`). The logs carry `Done 200 runs` per target, so it passed by running. **ci: run `34709219509` — `deny-check` green, `windows` fails at step 1 of 16** on a Phase-19 release blocker whose reason is still true — `DBT-P58-001`, owner-gated, and **not** removed or skipped. **release: queues forever**, 0 matching runners / 0 environments / 0 variables — `DBT-P58-003` |
| P58-3 | the icon regenerates from source | **CLOSED** — the source was never missing; P55 measured the wrong path. `--check` PASS, 17/17 byte-identical. `DBT-P55-005` |
| P58-4 | the two P47 product decisions | **CLOSED — both taken.** `DBT-P47-003` ACCEPTED out of scope (no consumer on any platform, and no platform populates a real per-core array). `DBT-P47-004` split: identity ACCEPTED out of scope (no consumer); VRAM total BLOCKED-MACHINE, with the wrong source named so the next session does not reach for it |
| P58-5/6/7 | the three Windows installer items | **BLOCKED-MACHINE** — the PC is unreachable from this Mac; evidence in `DBT-P55-008` |

---

## §2 Recovery posture — measured 2026-09-12

P49 recorded `Gate 0f` as regressed: "there is no `D:` and no
`D:\WindowsImageBackup`". **That is no longer true.** Measured today:

| artefact | state | evidence |
|---|---|---|
| external drive | **attached** — `HIKSEMI`, 953.9 GB, BusType `USB`, disk 1 | `Get-Disk` |
| `D:` volume | **present** — NTFS, label `SD`, 953.7 GB, 410.5 GB free | `Get-Volume` |
| `D:\WindowsImageBackup` | **present** — 19 files, 521.45 GB | `Get-ChildItem -Force -Recurse` |
| system image | **present and listable** — version `09/02/2026-09:46`, "Can recover: … Bare Metal Recovery, System State" | `wbadmin get versions -backupTarget:D:` exit 0 |
| image age | **10 days** (2026-09-02) — not today | as above |
| recovery media | **never boot-tested** | P47; unchanged |

**What is proven:** a listable, bare-metal-capable image exists, 10 days old.

**What is not proven:** that the machine can actually be recovered from it. The
recovery media has never been booted, so the path back has never been exercised
end to end.

**The constraint on re-imaging:** C: uses 571.6 GB; `D:` has 410.5 GB free. A
second full image does not fit beside the existing one. Writing one therefore
risks destroying the only verified image in order to attempt its replacement —
and if that attempt fails partway, the machine is left with no image at all,
which is strictly worse than today. This is `DBT-P55-003`.

**Item 3.B was not executed. Owner decision, 2026-09-12: do not re-image.** The
10-day-old image is kept rather than gambled. No `wbadmin start backup` was run
and `D:` was not written to. The gap this leaves — no image dated today — is
recorded rather than worked around, and it is the smaller of the two gaps: an
image ten days old still restores this machine, whereas the recovery media is
not attached at all (`DBT-P55-004`), so the path back has never been exercised
and currently could not be. **Fixing the media is what actually improves the
posture; re-imaging would not have.**

Nothing here clears Gate 4. Recovery remains **unproven**.

---

## §3 Owner-gated — what is blocked, and what each blocker unblocks

Absence of these is **not** missing work. Each is something no session can
supply.

| # | what the owner does | what it unblocks | why a session cannot |
|---|---|---|---|
| 1a | **Find and attach the recovery media.** It is not attached — `DBT-P55-004`. Nothing can be boot-tested until it exists on this machine | prerequisite for 1b | the media is physically elsewhere |
| 1b | **Boot-test the recovery media.** Boot from it once; confirm the recovery environment sees the system disk and `D:\WindowsImageBackup`. Then write `docs/phase55/RECOVERY-MEDIA-BOOT-TEST.md` — both gate scripts read that file as the record | Gate 4 entirely. Until this passes there is no *proven* way back from an unbootable machine | requires a physical reboot into WinRE and a human at the console |
| 2 | **Choose a Gate 4 candidate device** from `docs/phase55/GATE4-CANDIDATES.md`, and write its instance id to `docs/phase55/GATE4-CHOSEN-DEVICE.txt`. Preferred: attach the Canon G3010 printer and use it | Gate 4's driver install and rollback | a device choice is a risk acceptance, not a measurement |
| 3 | **Decide `DBT-P55-003`**: accept the 10-day-old image, or free space on `D:` / attach a second target so a fresh image can be written without destroying the existing one | Item 3.B's "a version dated today" | destroying the only verified image is not a session's call |
| 4 | Production Authenticode certificate | `QD-035-003`, and any signed release | it costs money and is an identity |
| 5 | **Generate and commit the dependency freeze set.** Run `scripts/freeze-dependencies.ps1 -Refresh` on the trusted dependency-freeze workstation, review the resulting graph, commit `release/dependency-locks.sha256`, `release/dependency-manifests.sha256` and `release/dependency-freeze.json`, and let the refresh clear `release/dependency-freeze.blocker.json` | `ci.yml`'s `windows` job, which currently fails at step 1 of 16 and reaches none of the other fifteen gates. `DBT-P58-001` | refreshing the freeze is an approval of the current dependency graph, not a measurement — and `DBT-P55-007` records that a developer machine cannot verify signature enforcement, so doing it on one would carry the weakness the register already names |
| 6 | **Decide `DBT-P47-004`'s VRAM half** once the Windows PC is reachable: wire `dedicated_total_bytes` from `HardwareInformation.qwMemorySize` or DXGI and prove the number traces to it, or accept the permanent `—` in the `perf.vram` tile | the only empty field in `§1` that a shipped screen actually renders | the measurement needs the machine, and `Win32_VideoController.AdapterRAM` — the obvious source — is wrong above 4 GiB |

---

## §4 The `QD-*` register

`DEBT_REGISTER.json` is a separate, append-only register under the p31 audit
gate (`aethercore.debt-register.v1`, ≥27 ids required). It is **not** merged
into §1 — the two use different id spaces and different closure rules.

Counted 2026-09-12: **42 entries — 21 `OPEN`, 19 `CARRIED`, 2 `CLOSED`.**

Verified against code this phase:

| id | claim | verdict |
|---|---|---|
| `QD-031-001` | gtk3-rs / unic-* advisories ignored in `deny.toml` pending tauri's gtk4 move | **accurate** — `deny.toml:13-25`, ten `RUSTSEC-2024-04xx` ids still listed |
| `QD-029-003` | SBOM is CycloneDX-*shaped*, honestly labeled not-certified | **accurate** — `SBOM.cdx.json`: `bomFormat CycloneDX`, `specVersion 1.5`, 600 components |
| `QD-035-001` | "Windows MSI/Burn install, upgrade, repair and uninstall runtime qualification deferred" | **partly stale** — P49 §49.5 installed *and* uninstalled the bundle on x64. Install and uninstall are qualified; **upgrade and repair are not.** The row should be narrowed, not closed |

The remaining 39 rows were **copied forward, not verified**. Most assert
something about an environment rather than about code ("macOS Intel spread
open", "no disposable SSH target on the proof host", "Postgres/MySQL lanes need
real servers") and cannot be adjudicated by reading the repository. Narrowing
them needs the machine each one names.

**Verified: 3. Copied forward: 39.**

---

## §5 What this ledger replaced, and the count

The `DBT-*` ledger was last reconciled in `SESSION_CONTEXT.md` §48.1, which
recounted every id and found **seven** rows that read `open` while the code said
otherwise. §49.9 then added four ids and moved five, and explicitly declined to
recount — "81 is arithmetic on §48.1's figure rather than an independent
recount". Phases 50–53 closed nine more ids across §51.8, §52.2 and §53.2.

P55's brief predicted "24 items currently read as open" and measured **15
rows**. P58's brief said "seventeen debt rows remain".

**Recounted by P58 from the table itself, by counting statuses, not by
arithmetic on a previous figure.**

Before (`d158bf5`): **23 `DBT-*` rows**, of which **17 have a status beginning
`OPEN`** — the brief's seventeen, exactly. The other six: 1 `ACCEPTED`,
2 `CLOSED`, 1 `RECLASSIFIED`, and 2 partials shown for traceability
(`DBT-P49-002` "3 of 4 FIXED", `DBT-P56-002` "NARROWED, not closed").

After P58: **29 rows**, of which **15 begin `OPEN`**. The delta is five out and
three in:

* out — `DBT-P55-001`, `DBT-P55-002`, `DBT-P55-005` closed with evidence
* out — `DBT-P47-003` decided to `ACCEPTED`; `DBT-P47-004` decided and split
* in — `DBT-P58-001` (`OPEN`, owner-gated), `DBT-P58-004` (`OPEN — low`) and
  `DBT-P58-005` (`OPEN`, the fail-open readers)
* neither — `DBT-P58-002` and `DBT-P58-006` were opened and closed in this
  phase, and `DBT-P58-003` was recorded straight to `ACCEPTED`

**Open: 17 → 15.** Two cautions against reading that as pure progress. First,
`DBT-P47-004`'s VRAM half left the `OPEN` count but is still work — it is
`BLOCKED-MACHINE`, not done. Second, three of the four new ids exist only
because the workflows became real enough to fail; the defects they name are not
new, they were simply unobservable while `DBT-P55-001` held — and `DBT-P58-005`
names a class that was always there and was found only because moving a file
made one instance of it visible. What genuinely improved is that three of the
fifteen now carry a named unblocking action in §3 instead of being undiagnosed.

Spot-checked as genuinely closed, so the next session does not redo them:

* `DBT-P50-001` — `total_space_bytes` / `free_space_bytes` present at
  `windows_impl.rs:771-781`
* `DBT-P50-005` — `PerformanceWindowResponse` present at
  `services/maintenance-service/src/performance.rs:365`
* `DBT-P51-003` — 15 `cap.reason.*` constants defined, 15 present in the UI
  catalogs: parity

Of the 29 rows in §1, **13 were verified against code, against the GitHub API,
or by running something this phase** — every row whose evidence column says
P58. The rest are machine-gated and carry their prior evidence. Three
(`DBT-P49-003`, `DBT-P55-008`, `DBT-P55-009`) are marked `BLOCKED-MACHINE`:
P58 could not attempt them because the Windows PC is not reachable from this
Mac, and the reachability itself was measured rather than assumed.
