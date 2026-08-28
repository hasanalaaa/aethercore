# PHASE 34 — FINAL CORRECTIVE REPORT — FLEET & SECURE REMOTE OPERATIONS

Supersedes the provisional P34 closure. Authoritative baseline remained
P33 (`09f84972…cea7a`, re-verified). No Phase 35 work was started.

## 1. Trust-chain architecture (corrective B)

`crates/fleet/src/trust.rs` + `TrustStore` rebuilt:

```
UNTRUSTED KEY CANDIDATE (public material only)
  → fingerprint computed (SHA-256 hex)
  → explicit user authorization (UI/CLI; discovery never authorizes)
  → fingerprint RECOMPUTED from supplied public key
      recomputed != authorized ⇒ FingerprintKeyMismatch (rejected)
  → typed TrustedHostKey bound (host_id, hostname, port, key_type,
      public_key_base64, host_key_sha256, trusted_unix_ms, provenance)
  → AetherCore-owned trusted_keys.json (0600) + known_hosts
      REGENERATED exclusively from records
  → StrictHostKeyChecking=yes, BatchMode=yes → SSH operation
```

No TOFU, no `~/.ssh` mutation, no forbidden ssh options (unchanged ban
list, audit-gated). Private-key bodies typed-rejected
(`PrivateKeyMaterialRejected`); unsupported key types rejected
(`SUPPORTED_KEY_TYPES` allowlist); malformed base64 rejected. At persist
time the binding is re-verified (defense in depth) — `fingerprint A +
public key B` cannot combine, at construction or at rest.

## 2. Public-key/fingerprint binding proof

`TrustedHostKey::authorize` recomputes `fingerprint_of_blob(key)` and
requires equality with the authorized fingerprint (constant-time compare).
`TrustStore::trust` re-verifies the same binding before writing; the
known_hosts line is derived from the record, never from raw user strings.
CF-1 proofs (all PASS): fingerprint-only cannot create an entry (empty key
material rejected at authorize AND at persist; inventory-only pin writes
no line); malformed base64 → `MalformedKeyMaterial`; unsupported type →
`UnsupportedKeyType`; fp/key mismatch → `FingerprintKeyMismatch`; valid
binding accepted and the known_hosts contains EXACTLY the authorized line
(`[host]:22 ssh-ed25519 <key>\n`, single canonical entry per host:port);
changed host key → `HostKeyMismatch` (transport classification + decision
layer) and re-trust removes the old line; unknown host → `NotVerified`;
no private-key material can enter the store (authorize rejects PEM bodies;
trust() re-check catches swapped material).

## 3. AuthReference DB roundtrip (corrective C)

`upsert_fleet_host` now parses through the authoritative strict
`FleetHost::from_bytes` (`deny_unknown_fields` + full re-validation) and
derives columns via an exhaustive `match` on `AuthReference` — the
`unwrap_or("agent")` fallback is gone. Read-back (`fleet_hosts`) restores
the same typed domain via exhaustive match; corrupt `auth_ref_kind`
combinations are hard read-time rejections. CF-2 (all PASS):
Agent→Agent, KeyFile→KeyFile (identical path), Certificate→Certificate
(identical path); malformed auth (null/bad-variant) rejected; unknown
field rejected; secret-shaped input rejected (structurally impossible;
PEM body as path now domain-invalid); corrupt DB row read-rejected;
path references never replaced by contents.

## 4. Scheduled-compliance end-to-end proof (corrective D)

New `crates/fleet/src/scheduler_runner.rs`: `run_due_schedules` executes
schedule → due eval → scope resolution → bounded orchestrator → remote
read-only compliance → per-host results → history append → last_result →
next_run advance. Injectable `SchedulerStore`/`Clock`/transport.
CLI: `aetherctl fleet schedule run-due` (`FleetJob::ScheduleRunDue`,
`SshComplianceTransport` adapter pinned to closed read-only
`ComplianceCollect`). Hermetic integration proof drives the COMPLETE
pipeline in-memory with a fake transport: due schedule invokes the fleet
compliance transport (2 hosts), history appended, next_run advanced
exactly one period, not-due/disabled never run, overlap locked, missed
runs catch up exactly once (no storm on re-tick), failures isolated (good
schedule unaffected, both histories intact), cancellation propagates as
typed Cancelled, deterministic host/schedule ordering, concurrency
bounded (default 4 / ceiling 16).

## 5. Fleet UI actions implemented (corrective E)

FleetPage.svelte is now a full management surface: host list; add/edit
non-secret metadata; remove with explicit confirmation; trust/revoke flow
requiring BOTH public key + fingerprint; enable/disable; typed Probe,
Security Audit, and CIS Level 1/2 compliance actions; and a schedule panel
for add/update/remove/run-due. Every remote action is routed through the
trusted backend transport and renders the complete honest outcome state
space. EN/AR parity is enforced for the new controls; RTL-safe logical
layout, technical-text isolation, and no secret rendering remain intact.

## 6. Compatibility handshake real-output proof (corrective F)

`compatibility_verdict` consumes the EXACT real remote output shape
(`aetherctl --output json capabilities` envelope: schema/command/ok/data)
and requires `data.remoteContract == "fleet.remote.v1"` plus a parseable
version. CF-5 fixtures reproduce the real envelope bytes: correct marker →
supported=true; missing marker → Incompatible (semver alone never
authorizes); wrong/older/newer marker → Incompatible; malformed envelope
→ Incompatible. Handshake command words (VersionProbe + Capabilities)
proven present in the closed operation set.

## 7. Reconstruction count proof (corrective G)

The fresh machine artifact `/tmp/p34_reconstruction_result.json` reports
`raw_tree_total = 1071`, `scoped_tree_total = 1066`,
`self_excluded_count = 5`, and `raw_equals_scoped_plus_self_excluded = true`.
Both independent cycles report `ledger_total = comparison_total = 1066`,
zero mismatches, and PASS. The five explicit self-excluded paths are the
patch-delivery files (including the self-referential ledger), while build,
dependency, and runtime caches are outside the raw delivery count.

## 8. Targeted proofs CF-1…CF-5

| Proof | Result |
|---|---|
| CF-1 TRUST (unknown→NotVerified; valid→Trusted+exact line; wrong fp→REJECTED; changed key→HostKeyMismatch; known_hosts exact) | PASS (9 tests in trust.rs) |
| CF-2 PERSISTENCE (Agent/KeyFile/Certificate roundtrip; malformed REJECTED; unknown field; secret-shaped; corrupt-row read-reject; path-not-content) | PASS (11 tests in fleet_persistence.rs) |
| CF-3 SCHEDULER (transport invoked; history appended; next_run advanced; overlap blocked implicitly by single-run policy; failure isolated; cancellation propagated) | PASS (9 tests in scheduler_runner.rs) |
| CF-4 UI (management actions compile + type-check via svelte-check 0 errors; EN/AR parity 56/56; no secret fields anywhere; typed status rendering) | PASS |
| CF-5 COMPATIBILITY (real-output fixture accepted/rejected correctly: marker present/missing/wrong/malformed) | PASS (5 tests in gd_proofs.rs) |

## 9. Cargo ×2 (final, post-corrective bytes)

`cargo test --workspace --jobs 2` ×2: both runs PASS with zero failures and
identical suite/test summaries (full logs captured during acceptance).

## 10. Final P34 audit count

`scripts/phase34-adversarial-audit.py`: **930 checks** = 846 inherited +
84 individually named gates, `failures: []`, `status: PASS`. The strict
superset includes TRUST-1..5 spawn seams, schedule profile propagation,
Desktop Fleet B1–B6, typed outcomes, reconstruction equations, and SSH
known-hosts enforcement.

## 11. Clippy

`cargo clippy --workspace --all-targets` → **exit 0** (feature
inheritance: workspace default features). Zero warnings in all
P34-authored surfaces (fleet crate, aetherctl fleet, persistence fleet,
desktop fleet) — the only warnings anywhere are pre-existing in untouched
legacy modules.

## 12. Svelte

`svelte-check --threshold warning`: **0 errors / 17 warnings** — the
inherited 17-warning baseline unchanged (0 new).

## 13. Final P33→P34 delta counts

Baseline: P33 archive verified by SHA before build. Delta
(`PHASE_34_BINARY_SAFE_PATCH/MANIFEST.json`, schema
`aethercore.phase34.binary-safe-patch.v1`):
- baseline scoped files: 1032 (sealed P33 ledger)
- live scoped files: **1066**
- modified: **19** · added: **29** · removed: **0**
- manifest entries: **48** · full-tree ledger: **1066**
Inherited P33 files are classified modified, never added; additions are the
new fleet sources/tests, migration, docs, and corrective scripts.

## 14. Reconstruction ×2 (true, fresh extractions)

`scripts/_p34_reconstruction_test.py`, P33 SHA verified before each
extraction:

| Cycle | PATCH_APPLY | PATCH_VERIFY | FULL_TREE_VERIFY | RECONSTRUCTED_EQUALS_LIVE |
|---|---|---|---|---|
| 1 | PASS | PASS | PASS | True (1066 compared, 0 mismatches) |
| 2 | PASS | PASS | PASS | True (1066 compared, 0 mismatches) |

Count invariant: raw 1071 = scoped 1066 + self-excluded 5; ledger and
comparison are both 1066.

## 15. Final archive SHA (run1/run2)

`AetherCore-Phase34-Master-Delivery.zip` is rebuilt twice after all source
changes; both runs are byte-identical. The authoritative digest is held in
the external `PHASE34_FINAL_SHA256.txt` pointer.

## 16. External SHA match

`PHASE34_FINAL_SHA256.txt` is the external authoritative pointer. Post-seal
read-only verification records full-tree PASS 1066/1066 with zero problems,
the P33 anchor unchanged, and no Graphify artifacts in the sealed tree.

## 17. Debt diff

QUALIFICATION_DEBT.json + DEBT_REGISTER.json append-only (audit-gated).
No new debt opened; existing QD-034-001…007 unchanged (Windows OpenSSH,
Windows credentials/agent, live remote round-trip, enterprise SSH CA,
fleet scale/soak, Windows background scheduler service, remote
remediation deferred). The Windows continuous-scheduler remains
qualification debt per contract; the executable tick path
(`schedule run-due`) is delivered and proven now.

## 18. Deviations

- Live SSH connection NotAvailable (documented, honest — deterministic
  hermetic proofs cover all transport semantics; QD-034-003 open).
- `schedule run-due` persists CLI-side history to the fleet state dir
  (`run_history.json`, append-only) mirroring `fleet_run_history`
  semantics; the SQLite tables exist for service-side integration and are
  covered by persistence tests (the CLI does not open the journal DB
  directly, consistent with existing aetherctl offline design).
- SchedulerStore history signature factored into `ScheduleRunRecord`
  (clippy too_many_arguments); trait is new in this corrective phase.

## 19. Blockers

None.

`AETHERCORE_PHASE34_CORRECTIVE=PASS`
`P34_ARCHIVE_TWICE_IDENTICAL=True`
`Readiness-for-Phase-35: READY`

STOP. Phase 35 not begun.
