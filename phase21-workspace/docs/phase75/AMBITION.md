# P75: the plan

AetherCore's claim is that every statement it makes is measured and every insight cites its
evidence. This phase moves the product toward that claim in parallel lanes, each on disjoint
files, each landing as one PR that is green on `main` before the next merges.

Written from `main` at `9747fe3`, green on CI run `35938070817` (head_sha `9747fe3`, measured).
The prompt said main was green there. At writing time the only green run was `35932036764` on
the PR head `2111657`, whose tree is byte-identical (`git rev-parse <sha>^{tree}`); the push run
on `9747fe3` finished green afterwards.

## 0. What Stage 0 measured before this plan

Eleven read-only audits covered disjoint partitions (ipc/security; the service; kernel/engine/
persistence; platform providers; intelligence; UI/desktop; installer/hardener/CLI; CI/scripts;
mutation crates; diagnosis crates; drivers/update/fleet). Their findings are hypotheses until a
lane reproduces them. Each finding below is labelled MEASURED or INFERRED as the auditor reported it.

Three facts about the ground itself:

* **`AUDIT/ASSESSMENT.md` does not exist.** `AUDIT/INVENTORY.md` (untracked) is a file inventory
  with no findings. There were no external findings to verify.
* **Workspace clippy fails on this Mac.** `cargo clippy --workspace --all-targets --locked --
  -D warnings` exits 101 on `9747fe3`, with dead code in `driver-backup` and four lints in
  `performance-telemetry/src/macos_impl.rs`. CI never sees this because clippy runs only on
  Windows. `cargo fmt --check` and `cargo test --workspace --locked` pass here (MEASURED).
* **Ledger §1:** 76 rows, 23 open, 3 malformed (`DBT-P56-002`, `DBT-P63-010`, `DBT-P63-014`),
  counted with the rule in `docs/LEDGER.md`.

New defects that shape the waves (all new unless a ledger id is given):

| sev | where | what | basis |
|---|---|---|---|
| high | `driver-hub/src/lib.rs:656-706` → `windows-update/src/windows_impl.rs:139` | The idle scheduler runs an **online** Windows Update search every ~12 h with no user action. This breaks "no network at rest". | MEASURED (code path) |
| high | `installer/wix/Product.wxs:344` | `PurgeMachineData` runs on `REMOVE="ALL"` without `NOT UPGRADINGPRODUCTCODE`, so a major upgrade deletes all of `%ProgramData%\AetherCore`. | INFERRED (MSI semantics) |
| high | `scripts/build-installer.ps1:142-150` | `Deterministic-ProductCode([string]$input)` binds the automatic variable `$input`. Every version may get the same ProductCode. | INFERRED |
| high | `apps/aetherctl/src/offline.rs:~596` | `keys generate` seeds an Ed25519 key from the literal `/dev/urandom`. On Windows, any user who creates `C:\dev\urandom` chooses an administrator's key seed. | MEASURED (code) |
| high | `intelligence-core/src/engine.rs:316-358` | Fallback runs only when the model returns zero *raw* candidates. If every model insight fails the citation gate, the user gets none. | MEASURED |
| high | `maintenance-service/src/intelligence.rs:109-118` | Raw timeline events are packed as `TimelinePattern`. The fallback then claims recurring patterns at Strong confidence. | MEASURED |
| high | `persistence` + `cleaner` + `system-repair` + `startup-manager` | `DBT-P63-012` is wider than the ledger says. No API transitions and journals atomically, and a crash in the window makes `recovery_required=false` permanent, because terminal plans are never recovered. | MEASURED order, INFERRED crash |
| critical | `care-orchestrator/src/{engine,model}.rs` + `maintenance-service/src/care.rs` | One-Click Care runs ReviewOnly steps (driver install, system repair) under one boolean session consent, sorts them first, binds no digest, and takes a private lease that is not machine-wide. | MEASURED |
| high | `maintenance-service/src/composition.rs:249-355` | Care decides which domain owns a plan through `status()`, which answers only for started plans. Every non-empty care run fails at step 1. One `CancelCareRun` revokes the fence until restart. | INFERRED |
| high | `crash-diagnostics/src/windows_impl.rs:129-162` + `diagnostic-engine/src/lib.rs:746-753` | If the Event Log query fails, the card still says "no logged memory errors in the last 30 days". Minidumps are never age-filtered. Kernel-Power events of every id fill a 128-event cap. | MEASURED |
| high | `security-audit/src/filesystem.rs:42-121`, `lib.rs:254-289` | `DBT-P36-006`, confirmed: on Windows every writable file gets synthetic mode 0666 and is flagged world-writable at Exact confidence. Files directly under a scan root are never examined. The CVE lane reports `Ok(0)` when the census is unsupported. | MEASURED |
| high | `performance-telemetry/src/{linux,macos,windows}_impl.rs` | Readings that measure something other than their label: Linux queue depth is a cumulative counter, macOS disk "active time" is disk fullness, Windows GPU busy is one arbitrary process's, Windows fault rates are paging counters. | MEASURED |
| high | `update-download/src/lib.rs:40` | A 60 s whole-request timeout means any realistically sized update fails to download. | INFERRED |
| high | `apps/ui/src/lib/policy.ts:24` | Every page says "Denied by policy: outbound network". Update check and Fleet do make outbound connections on user action. The claim is false. | MEASURED |
| high | `apps/desktop/src/main.rs:2238-2909` | 13 `fleet_*` Tauri commands are synchronous with 45 s SSH timeouts, so they freeze the window. | INFERRED |
| high | `persistence/src/export.rs:269-333` | `export verify` trusts the public key embedded in the file it verifies, so a forged, re-signed export reports `verified:true`. The header is outside the signature. | MEASURED |
| high | `windows-installer.yml:399-431` | The probe's rollback gate cannot fail usefully. It samples service state after `sc stop`, and SID type and elapsed time are recorded but never gated. | MEASURED |
| high | `static_validate.py:1842-1843`, `enterprise-adversarial-audit.py:180`, `zenith-recursive-audit.py:455`, `phase10-architecture-audit.ps1:181` | Negative checks that rustfmt's layout (or a missing `(?m)`) makes unmatchable, so they pass whatever the code says. | MEASURED |

The auditors also recorded about 60 medium and low findings. The lanes below absorb them by
file. The rest are in §4.

## 1. Rules every lane follows

The binding text is the lane contract given to every lane agent. In short:

* One lane equals one PR from `lane/<name>`, touching only the files listed for it.
* Reproduce, change, prove. A behavioural change adds a test that fails without it.
* One concern per commit. Each commit message says what, why, and the proof.
* Run the local gates before every push: fmt, clippy (macOS, and the Windows target where the
  crate graph has no C build script), tests, `static_validate.py`, `test_gate_readers.py`,
  `ps_marker_scan.py`, and the audits that name touched files.
* Seal with `source_seal.py --json` first. Every listed path must be one you changed. Then
  explicit `git add`, `regenerate-source-manifest.py`, `git add MANIFEST.sha256`, and
  `source_seal.py` must print OK.
* Lanes do not edit `docs/LEDGER.md`. Each writes `docs/phase75/lanes/<name>.md`. The lead
  moves the ledger rows on the PR branch at merge time, so the row still moves in the same
  squashed commit as the work.
* No lockfile change outside the dependency lane.
* Merges happen one at a time. Before each merge: update the branch, re-seal, move the ledger,
  and get a CI run green whose head_sha equals the head being merged.
* Windows behaviour is proven on the `windows-2025` runner (`ci.yml`, or `windows-installer.yml`'s
  `bundle-log-acl-probe`), never asserted.

## 2. Wave 1 — highest value (running)

| lane | ledger | files | acceptance test | proof |
|---|---|---|---|---|
| `service-rollback` | `DBT-P74-001`, then `DBT-P74-002`; upgrade purge; ProductCode | `installer/wix/Product.wxs`, `apps/install-hardener/**`, `scripts/build-installer.ps1`, `.github/workflows/windows-installer.yml` | After a forced rollback, the probe gate reads the restored service `RUNNING` with `SERVICE_SID_TYPE: UNRESTRICTED`, and the rollback takes < 60 s. The gate is sampled before any stop and is shown red on unchanged product code. | probe run ids, red then green; CI at PR head |
| `insight-model` | `DBT-P56-002`, `DBT-P62-004` | `crates/intelligence-core/**`; service `intelligence.rs`, `assistant.rs`, the model block of `composition.rs`; UI catalog keys | Real-model insight test: `Ok`, every citation resolves, engine `LocalModel`, done before the deadline. Selector test: all uncitable → fallback present. `ASSISTANT_DEADLINE` unchanged. | local Metal timings; Windows CI log's measured insight and assistant wall times |
| `plan-journal` | `DBT-P63-012` (+ startup-manager, streaming watchers) | `persistence/src/lib.rs` (not care or export), `operation-engine/**`, `cleaner/**`, `system-repair` lib and tests, `startup-manager` lib, `driver-install` lib, service `streaming.rs` | A SQLite trigger captures the journal row at the instant the state flips. The terminal state carries `recovery_required=1`. An aborted journal write leaves the plan non-terminal. Red today. | macOS tests; Windows CI |
| `no-egress` | new | `driver-hub/**`, `windows-update/**` (no signature change that forces edits in other lanes), service `scheduler.rs` | The passive scan requests `LocalCacheOnly` and the interactive scan requests `Online`. Red today. | test, Microsoft's documented `Online=false` semantics cited; CI |
| `mac-clippy` | `DBT-P63-010` | files carrying a macOS clippy finding | Workspace clippy `-D warnings` exits 0 on macOS | local; Windows-target clippy; CI |
| `keygen-rng` | new | `apps/aetherctl/src/offline.rs`, `apps/aetherctl/tests/**` | Windows test: a planted `\dev\urandom` does not become the seed. An existing key is never overwritten. Unix mode is 0600. No `Cargo.lock` change. | CI (the Windows test runs in `cargo test`) |

Merge order is whatever turns green first, with `mac-clippy` first if it is ready.

## 3. Wave 2 — starts from `main` after Wave 1 merges

| lane | ledger / finding | files |
|---|---|---|
| `installer-ux` | `DBT-P55-009` (theme order: Uninstall before Repair; repair stays available, per `static_validate.py`'s `phase8_msi_repair_not_disabled`); `DBT-P55-008` (`.wxl` for 1025, RTL-aware theme, `-culture` in the build; on the runner try `Set-WinUILanguageOverride` after installing ar-SA, and if the runner cannot represent it, say so); launch conditions without `Installed OR` | `installer/wix/Bundle.wxs`, `installer/wix/theme/**`, `Product.wxs` strings, `build-installer.ps1`, `windows-installer.yml` (UIA focus gate) |
| `care-consent` | care runs ReviewOnly steps; ordering; digest binding; private lease; dispatch by `status()`; permanent fence revoke; unbounded status loop; `replace_care_steps` deletes every run's history | `crates/care-orchestrator/**`, service `care.rs`, `router/care.rs`, the care part of `composition.rs`, the care functions in `persistence` |
| `diagnosis-evidence` | Event Log failure reported as "no errors"; Kernel-Power noise and cap; minidump age; Ready with faults; single-sample GPU root cause; I/O evidence citing the peak | `crates/crash-diagnostics/**`, `crates/diagnostic-engine/**`, `crates/performance-bottleneck/**` |
| `db-diagnostics` | MySQL seconds reported as ms; PostgreSQL last-wins; `synchronous_commit`; MySQL comment, flag and section parsing; slow-log "outlier" never computed | `crates/db-diagnostics/**` |
| `fs-acl` | `DBT-P36-006` (a real DACL walk with `GetNamedSecurityInfoW`; world-writable means a write-class ACE for Everyone, AU, Users or Anonymous, or a NULL DACL); root-level files skipped; CVE `Ok(0)` on an unsupported census; firewall state from the registry | `crates/security-audit/**` |
| `telemetry-windows` | `DBT-P49-004` (`intervalMs` equals the window measured); `DBT-P47-003` (per-processor via PDH wildcard); GPU per-process versus adapter; fault counters; `MhzLimit` units; letterless-disk capacity; `% Disk Time` versus `% Idle Time`; collectors under the runtime timeout | `crates/performance-telemetry/src/{windows_impl,lib}.rs` |
| `fuzz` | `DBT-P58-004`: all 12 targets built and run by `fuzz.yml` (the `update_manifest` target needs crate-root re-exports) | `fuzz/**`, `.github/workflows/fuzz.yml`, `update-engine/src/lib.rs` re-exports only |
| `gate-honesty` | `DBT-P63-009`, `DBT-P63-014`, `DBT-P65-003`; the vacuous negatives in §0; workflow-wiring checks that pass on commented-out text; PyYAML absent means the parse check passes; `test_*` meta-tests and `ps_marker_scan.py` have no CI caller. Each fix shows the gate failing on a planted regression. | `scripts/*.py` gates, `scripts/*-audit.ps1`, `.github/workflows/ci.yml` |
| `service-host` | `DBT-P60-003` (POSIX state dir); log init failing before the dispatcher (1053, no log); no `STOP_PENDING`; `RUNNING` before composition; `register()` failure leaving `START_PENDING`; the Windows log never rotates while running | service `main.rs`, `windows_service_host.rs`, `unix_service.rs`, `crates/diagnostics/**`, `apps/aetherctl/src/transport.rs` |
| `update-trust` | download timeout; staged-exe failure blocking service start for 2 h; support-bundle redaction leaks (escaped paths, host, IP, MAC, serials); release-authority ignoring key expiry, `1.2` versus `1.2.0`, offline `..` | `update-download/**`, `update-engine/src/{coordinator,platform}.rs`, `support-bundle/**`, `release-authority/**` |
| `ui-truth` | policy-band network claim; Ctrl+Shift+digit shortcuts dead; assistant live region and focus return; a11y labels; Fleet sync commands freezing the window; Fleet errors shown as success; backend prose raw in Arabic; insights and assistant sending no locale | `apps/ui/**` (not the catalog keys added in Wave 1), `apps/desktop/src/main.rs` |
| `cli-truth` | `export verify` trusting the embedded key (report the signer fingerprint; `verified` only against a supplied trusted key; the header outside the signature is recorded, because changing EXPORT_V1's digest is a published format and needs the owner); `update verify`'s self-referential "verified"; non-atomic `vulndb update`; `--format both`; `care start` accepting a 1-character prefix; `--lang ar` parsed and dropped | `persistence/src/export.rs`, `apps/aetherctl/src/{release,sec,service_cmds,main,i18n,cli}.rs` |
| `seal-root` | `DBT-P60-002`: the root `.github/**` sealed. It changes every lane's seal, so it merges last in the wave. | `scripts/source_seal.py`, `regenerate-source-manifest.py`, `omega-evidence.py`, `docs/SOURCE_SEAL.md` |

`DBT-P49-004`, `DBT-P60-002` and `DBT-P60-003` sit in `telemetry-windows`, `seal-root` and
`service-host`.

## 4. Wave 3 — depth

* **Providers parity** (macOS/Linux, `performance-telemetry/src/{macos,linux}_impl.rs`,
  `hardware-telemetry`). Per-core arrays currently hold the total. macOS and Linux fault rates
  read 0 with no fault. Linux thermal uses the wrong sysfs path. Linux capacity is taken from "/"
  for every device. An hours-long CPU average is published as current. Two samplers can run after
  a stop and start. Each gap closes with the platform API named in the audit's parity table
  (`host_processor_info`, `/proc/vmstat`, `/sys/class/thermal/…`), or is reported as a fault,
  never as a zero.
* **Evidence breadth.** Feed `pc-intelligence` findings and real `TimelineBuilder` patterns into
  the insight pack, so the model reasons over bottleneck, repair and security evidence, not only
  history.
* **Performance and memory under load.** Tables that are never pruned, and `plans(state)` with
  no index. Pruning deletes history, which is persisted data, so it goes to the owner before any
  migration. The idle probe runs WMI every 5 s whether or not work is eligible, and the UsoSvc
  "servicing" signal is always busy.
* **IPC.** A zombie client reader after shutdown; hello with no deadline; `/tmp/aethercore-ipc`
  squattable on Unix; the dev SDDL.
* **UI/UX and accessibility at the level of the best software on any platform.** EN/AR parity is
  already exact at 1,694 keys each. The remaining work is contrast, RTL motion and keyboard
  layouts.
* **The CLI as a first-class surface.** Arabic output, a truthful `doctor`, and liveness instead
  of PID assumptions.
* **Docs a newcomer can build from:** `docs/BUILD.md` verified by running it cold.
* **Dependency lane** (the only lane allowed to change lockfiles): `DBT-P70-005` (TypeScript 7
  needs TS 6 alongside and `svelte-check --tsgo`), plus any open bumps, in one branch, with the
  freeze minted there (`docs/phase70/P70-REPORT.md` §3).
* Driver downgrade protection; Fleet `ssh -i` quoting; the `performance-optimization` governor
  (its `running` slot is never cleared, and it ignores consent and digest, though it is latent
  behind `NoopPlatform`).

## 5. Out of scope — record only

`DBT-P55-003` (D: imaging capacity), `DBT-P55-004` (recovery media), `DBT-P41-001` (VC++ install
branch, needs a machine without it), `DBT-P55-006` (ARM64 recipes), `DBT-P55-007` (signing and
NuGet trust on the developer machine). No certificate, no physical PC and no ARM64 VM are
available to this phase.

## 6. What done means

A lane is done when it is merged to `main` and a CI run with head_sha equal to that merge is
green. The report `docs/phase75/P75-REPORT.md` then pastes, per lane: `cargo fmt --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace
--locked`, `static_validate.py`, `test_gate_readers.py`, `ps_marker_scan.py` (including its
UNMEASURED lines), `source_seal.py`, and the CI run id. A claim without the command that
produced it is labelled a belief.
