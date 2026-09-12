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

Last moved: P56 (2026-09-12).

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
| `DBT-P47-003` | `perProcessorBusyBp` is never populated on Windows | OPEN — product decision | verified P55: `crates/performance-telemetry/src/windows_impl.rs:423` → `per_processor_busy_bp: Vec::new()` | any Windows |
| `DBT-P47-004` | GPU adapter identity and VRAM stay empty; no honest source wired | OPEN | verified P55: `windows_impl.rs:973` — "adapter_id, adapter_name and dedicated_total_bytes stay" empty | any Windows |
| `DBT-P49-002` | the consumer installer's UI | **3 of 4 FIXED and photographed; version NOT fixed** | `docs/phase55/P55-ITEM4C-LIFECYCLE.md` + `installer-screens/`. Title bar now "AetherCore Setup"; Æ mark renders; progress reads "Processing: AetherCore". **Version still absent — cause found: the shipped `thm.xml` has no version control at all, so `ShowVersion=1` has nothing to act on; fixing it needs a custom `ThemeFile`.** Licence still absent, deliberately | this machine |
| `DBT-P55-007` | **`signatureValidationMode=require` is not enforced on this developer machine.** A cold `dotnet tool restore` with deliberately corrupted `trustedSigners` fingerprints still succeeded (exit 0), so the supply-chain control the project believes it has does nothing here. The same restore is enforced on the CI runner, which is where NU3034 surfaced. Pre-existing, not introduced by P55 — but it means **no local build can verify this control**, and a developer-machine build cannot be treated as evidence that packaging dependencies were signature-checked | OPEN — security-relevant | verified P55 by negative control: fingerprints replaced with `0000…`/`1111…`, restore still exit 0 on SDK 8.0.424 | any dev machine |
| `DBT-P55-005` | `apps/desktop/icons/SOURCE.json` names `design/icon/aethercore-mark.svg` as the source of the shipped icon set, and **that file is not in the tree**. The generated PNGs match their recorded hashes, so the artwork is authentic, but the set cannot be regenerated from source | OPEN | verified P55: `Test-Path design/icon/aethercore-mark.svg` → False; `64x64.png` and `128x128.png` both match `SOURCE.json` | any |
| `DBT-P49-003` | the bundle leaves an elevated log in `%TEMP%` after install and uninstall | OPEN — **reproduced, and its filename pattern corrected** | verified P55 Item 4.C: three survivors, `%TEMP%AetherCore_20260912{125833,130229,130850}.elevated.log`, 920/929/929 B. **P49 recorded `AetherCore_Setup_*`; they are now `AetherCore_*`** because the bundle Name changed and Burn derives the log name from it — a sweep written against the old pattern would report zero and be wrong | this machine |
| `DBT-P49-004` | `intervalMs` does not describe the window the numbers were measured over | OPEN | verified P55: `windows_impl.rs:349` `let _ = interval;`, `:369` `from_millis(120).min(from_millis(100))` (the `120` is dead), `:710` hardcoded 80 ms, `lib.rs:330` publishes the *requested* value | any Windows |
| `DBT-P55-001` | `phase21-workspace/.github/workflows/{ci,fuzz,release}.yml` have **never been dispatched**. GitHub executes only workflows under the repository root `.github`; these sit one directory down | OPEN | verified P55: `git ls-files '*.github/workflows/*'` — only `.github/workflows/windows-installer.yml` is at the root. First recorded by P54 | — |
| `DBT-P55-002` | `cargo fmt --all -- --check` fails on **1,247 files**. `ci.yml`'s formatting gate would have caught the first one — direct evidence that `DBT-P55-001` has held for the whole life of that file | OPEN | verified P55: 1,247 distinct files in the diff | any |
| `DBT-P55-003` | `D:` cannot hold a second full system image beside the existing one: C: uses 571.6 GB, `D:` has 410.5 GB free | OPEN | verified P55 by enumeration, see §2 | this machine |
| `DBT-P55-004` | the recovery media is **not attached** — zero removable volumes carry `\sources\boot.wim` or `\bootmgr`. It is not merely un-boot-tested; it is absent, so it cannot be tested until it is found | OPEN | verified P55: `scripts/gate4-preconditions.ps1`, condition 3 | this machine |
| `DBT-P55-008` | the installer rendered **English only**, on a machine whose Windows UI language is Arabic (the shell's own windows render Arabic here). Whether the bundle supports Arabic was **not measured** and is not claimed either way | OPEN | P55 Item 4.D, 80 captured frames, all English | this machine |
| `DBT-P55-009` | launched with `/uninstall`, the bundle still presents the "Modify Setup" chooser, and **Repair is the focused default**. Enter takes Repair, not Uninstall | OPEN — usability | P55 Item 4.C: an unplanned repair ran to completion this way, `installer-screens/07-repair-complete-unplanned.png` | this machine |
| `DBT-P49-001` | `build-release.ps1` never set the ADK `DismApi\Lib\amd64` path into `LIB`, so a clean shell died `LNK1181` | **CLOSED** | `07445b4` — `crates/system-repair/build.rs` locates the library itself; the failure was reproduced first (exit 101, `LNK1181` ×3 with `LIB` unset), then measured at exit 0 | done |
| `DBT-P55-006` | two recorded ARM64 recipes still set `LIB` to the DismApi path by hand — `scripts/build-arm64-msi.cmd:77` and `scripts/p36vm/p36_relbuild.cmd:11`. Now redundant: `build.rs` resolves `arm64` the same way it resolves `amd64`. Harmless (build.rs treats existing `LIB` entries as a fallback), so they were **left alone rather than changed**: both are documented verbatim reproduction recipes for a VM this session cannot test on | OPEN — low | verified P55 by grep; retire them on the ARM64 VM with a build that proves the recipe still works without the line | the ARM64 VM |
| `DBT-P56-001` | commit `2be8396` is titled **"finish local AI chat"** and delivered no chat. The AI-facing wire verbs are `ListInsights`, `RequestInsight`, `DismissInsight` and nothing else | OPEN — recorded so the message is not trusted | verified P56: `grep -c chat services/maintenance-service/src/*.rs` → 0 on all 14 files | any |
| `DBT-P56-002` | **the embedded model had never generated a token.** `LlamaCppReasoner::infer_embedded` returned `Err` unconditionally, so every request degraded to `DeterministicFallbackReasoner` while `engine_label` still read `localModel` | **NARROWED, not closed.** The ASSISTANT path generates: `StreamingReasoner` is implemented over llama.cpp with a greedy sampler, a 1.15/256 repetition penalty, a 512-token ceiling, a 20 s deadline and a cancel flag checked between tokens. Measured on this machine: 82–138 tokens in 0.24–1.2 s, Metal, 151 MiB compute buffer; `tests/embedded_generation.rs` 5/5 green against the real artifact. **The INSIGHT path is unchanged** — `infer_embedded` still returns that same `Err` at `llama.rs:237` and insights still come from the rule engine | any |
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
| P56-1.D | present it — screenshots, both languages, both themes, populated and empty. **Gate: no roll-out until the owner approves** | **AWAITING THE OWNER** — `docs/phase56/screens/{populated,empty}/overview-1280-{en,ar}-{dark,light}.png`, 8 images |
| P56-2.A | the chat's interaction design | **CLOSED** — `DIRECTION.md` Part 2.A: an inline-end drawer, three named outcomes, the wire stated before implementation |
| P56-2.B | the wire contract | **CLOSED** — `crates/contracts/proto/assistant.proto`; request tags 92/93, response 92, `EVENT_KIND_ASSISTANT_TURN` = 30, envelope payload 39. Shape stated in `DIRECTION.md` before implementation |
| P56-2.C | the service side | **CLOSED** — `services/maintenance-service/src/assistant.rs`: worker thread, 120 ms stream throttle, per-principal cancel registry, boundary validation, and a fault path that never yields an empty answer. Router arms for both verbs; `AssistantCoordinator` in `ServiceContext` |
| P56-2.D | the UI | OPEN |
| P56-2.E | the tests that would catch the failure that matters | **RED, committed first** — 14 green in `assistant.rs` (ungrounded answer refused, fault not empty string, cancellation), 2 RED in `tests/embedded_generation.rs` against the real artifact. 2.C turns them green |
| P56-3 | roll the direction out to the remaining screens | BLOCKED on P56-1.D |

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

P55's brief predicted "24 items currently read as open". The measured figure is
**15 rows** in §1: 10 carried `OPEN`, 1 `ACCEPTED`, 3 new this phase, plus 1
closed and 1 reclassified shown for traceability. The difference is mostly
phases 50–53 — all eight `DBT-P50-*`/`DBT-P51-*` ids and `DBT-P48-001` closed
after §49.9 was written, and `DBT-P42-011` stopped reproducing.

Spot-checked as genuinely closed, so the next session does not redo them:

* `DBT-P50-001` — `total_space_bytes` / `free_space_bytes` present at
  `windows_impl.rs:771-781`
* `DBT-P50-005` — `PerformanceWindowResponse` present at
  `services/maintenance-service/src/performance.rs:365`
* `DBT-P51-003` — 15 `cap.reason.*` constants defined, 15 present in the UI
  catalogs: parity

Of the 15 rows in §1, **10 were verified against code or by measurement this
phase** (their evidence column says "verified P55"); the rest are machine-gated
and carry their prior evidence.
