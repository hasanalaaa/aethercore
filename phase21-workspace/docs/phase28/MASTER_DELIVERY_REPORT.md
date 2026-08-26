# AetherCore — Phase 28 Master Delivery Report
# Headless Command Surface: aetherctl CLI + Service Lifecycle Managers

Status: COMPLETE on this host (macOS/ARM, Apple Silicon — the unix test host per CX-7).
Windows behavior FROZEN: zero cfg(windows) files modified; the aetherctl Windows lane
reuses the frozen `aethercore_ipc::SessionClient` verbatim (QD-028-001 records the lack
of Windows-host proof honestly).

## Scope executed

T1 apps/aetherctl (new binary crate): hand-rolled parser (no clap), global flags before
the command only (`--socket-dir --timeout-ms --output json|text --no-color --version`),
versioned machine envelope `aethercore.aetherctl.v1` on every JSON output with strict
`deny_unknown_fields` parse-back before stdout, ASCII aligned text tables derived from the
same JSON projection, locale-independent formatting.
T2 transport reuse (CX-3): unix → `UnixSocketSession` + additive `set_io_timeouts()`
(hang-proof within --timeout-ms); windows → frozen named-pipe client reused unchanged.
Detection tri-state Reachable{pid} / Offline / StaleEndpointRecovered via socket probe +
PID-file liveness cross-check (kill(pid,0); cfg(unix) libc under owner-review waiver).
Offline + service command ⇒ typed exit 3, always inside the deadline.
T3 embedded offline commands, strictly read-only: about/version · capabilities (static
platform-capabilities matrix) · engine-source (selection parity asserted against the
service composition by gate p28-engine-source-parity) · telemetry-once (ONE real snapshot
from the cfg-selected PerfPlatform provider; degraded collectors surface as typed
CollectorFaults, nothing simulated) · self-check (manifest schema validation + streaming
sha256 fail-closed; actual model loading ONLY behind --load-model and additionally behind
an opt-in cargo feature, otherwise typed exit 7) · service detect · service units --print
(ARTIFACTS ONLY — writes nothing). Mutation-guard audit scan proves ZERO mutation-capable
symbols in the embedded code paths.
T4 service-backed set over EXISTING tags only: doctor→40 · perf 71/72/73/74 · optimize
75/76/77 · timeline 78/79 · care 82/80/83/81 · insights 84/85/86 · scan 65/66/67/68.
Consent discipline: digest printed before asking; interactive confirm requires retyping
the digest prefix (default NO; wrong digest refused BEFORE any grant RPC); grant+start
executed by THIS principal connection in-process; --non-interactive and JSON mode NEVER
auto-consent (typed exit 6). ROUTING_TABLE.md maps every subcommand ↔ tags ↔ exit codes ↔
offline availability plus an honest "Deferred RPC lanes" section (updates 46–52/58–64,
driver-policy 69–70, support-bundle 53–57, generic consent 42–44).
T5 exit-code registry as single enum + EXIT_CODES.md + test pins; tri-equality enforced
(0 ok · 2 usage · 3 unreachable · 4 timeout · 5 rejected-by-service · 6 consent ·
7 capability · 8 local io · 130 SIGINT).
T6 lifecycle closure (QD-026-003): SIGTERM/SIGINT → stop accepting → bounded
GRACEFUL_DRAIN_WINDOW lets connected sessions finish the single in-flight frame → socket
file removed → PID file removed → exit 0. Proven LIVE (see T7). --daemon now writes
size-capped rotating JSONL logs (5 MiB cap constant, keep-last-N=3, deterministic names,
runtime rolling; Windows init_json_file untouched). packaging/launchd plist validated
LIVE via plutil -lint (= OK); packaging/systemd unit statically asserted
(Type/ExecStart--daemon/Restart/hardening keys) with honest NOT_EXECUTED note
(QD-028-002). Nothing auto-installs anything (CX-5).
T7 integration proof suite (real spawned daemon, this Mac):
offline_surface_succeeds_with_no_daemon_present ·
service_backed_command_matrix_round_trips_over_real_uds ·
kill9_stale_endpoint_detected_then_next_daemon_rebinds (SIGKILL → StaleEndpointRecovered
→ typed exit 3 → next daemon rebinds SAME rendezvous → Reachable{pid}) ·
sigterm_drains_connected_session_then_exits_zero_cleaning_files (drain protects the held
session; release → exit 0; socket+PID removed) ·
consent_adversarial_refusals_are_typed_and_grant_is_visible ·
service_set_fails_fast_offline_within_timeout (<2 s).
T8 scripts/phase28-adversarial-audit.py — strict superset importing phase27's chain:
exit-code↔docs↔enum tri-equality · routing-table coverage against REAL proto tags
(invented tag = failure) · mutation-guard symbol scan of embedded paths · dependency ban
allowlist for apps/aetherctl (serde/serde_json/sha2/thiserror + wire-contract path deps +
prost waiver + cfg(unix) libc waiver; network crates banned) · live plutil -lint recorded ·
systemd static keys + NOT_EXECUTED honesty · wire freeze (requests ≤88, responses ≤50,
EventKind ≤28, envelope ≤37) · renderer byte-untouched vs sealed P27 snapshot ·
cfg(windows) freeze continues · engine-source parity · QD-026-003 closure evidence.

## Inherited defect fixed this phase

P28-I-007 (medium): composition.rs wired PerformanceEngine::with_synthetic() on the unix
path while engine_source() reported "native" — simulated counters labeled as truth,
exposed headlessly by the new perf lanes. Fixed: default_platform() (REAL cfg-selected
provider) is the composition default; synthetic remains ONLY behind force-synthetic-perf.
Live transcript after the fix shows genuine macOS samples (fileproviderd/bird process
rows, honest Degraded faults reading "not simulated").

## Environmental incident (recorded, not agent-caused)

An outside sync-style process injected 30 macOS-style duplicate entries mid-session
("lib 2.rs", "phase26_unix 2.rs", "Cargo 2.toml", "new-files 2/", …), one of which broke
cargo target discovery. None exist in the sealed P27 archive; all were QUARANTINED to
/tmp/p28_quarantine with a sha256+timestamp manifest (docs/phase28/ISSUES.json P28-I-006).
Operators should investigate the host-side syncing process.

## Gate tails

GA. Baseline gates unchanged pre-change (G0-a PASS/48 · G0-b whole-tree IDENTICAL to the
extracted sealed archive after normalizing two post-archive patch-regeneration files and
python __pycache__ execution artifacts · G0-c FINAL {"checks":421,"failures":[]} ·
G0-d 394 passed ×2). Post-change whole-tree digest differs from the sealed anchor ONLY on
intentional Phase-28 additions/modifications (exact count + list in the standardized
status report; runtime state dirs and python caches excluded as execution artifacts).
GB. `cargo test --workspace --jobs 2` ×2 identical: **409 passed / 0 failed** both runs;
with proof-suite features
(`--features aethercore-maintenance-service/unix-ipc,aetherctl/e2e`) ×2 identical:
**420 passed / 0 failed** both runs (>394 baseline; delta = aetherctl units + diagnostics
rotation tests + cli tests + GD/P27 unix-ipc test + 6 Phase 28 real-daemon proofs).
GC. phase28-adversarial-audit → PASS, **572 checks** (>421 inherited), failures=[].
GD. Real-daemon proof suite green (6/6, names above) incl. SIGTERM graceful drain→exit 0→
file cleanup and the consent adversarial set.
GE. Live transcript pasted in the standardized status report: ≥5 verbatim envelopes over
the real UDS incl. envelope header lines ("schema":"aethercore.aetherctl.v1").
GF. svelte-check: 0 errors / 17 warnings (unchanged; renderer byte-compared against the
sealed snapshot).
GG. PHASE_28_BINARY_SAFE_PATCH built from the sealed-P27-snapshot diff; apply → verify
round-trip byte-identical ×2 (script prints ARCHIVE_TWICE_IDENTICAL analog per run).
GH. Deterministic master archive AetherCore-Phase28-Master-Delivery.zip rebuilt twice,
identical SHA-256 (+PHASE28_FINAL_SHA256.txt next to the archive, outside it).
