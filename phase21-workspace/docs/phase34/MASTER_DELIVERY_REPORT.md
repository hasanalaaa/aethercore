# Phase 34 — Master Delivery Report — Fleet & Secure Remote Operations

## Corrective closure (final)

This report supersedes the provisional P34 closure (the earlier archive,
identified by its hash in the external history file, is SUPERSEDED — the
authoritative archive hash is only ever read from
`PHASE34_FINAL_SHA256.txt`). The five supervisor closure items were
implemented and re-proven end-to-end; all authoritative gates were re-run.

## 1. Trust chain (corrective B)

End-to-end chain proven in `crates/fleet/src/trust.rs` and
`docs/phase34/FLEET_TRUST.md`: untrusted candidate → fingerprint computed →
explicit user authorization → fingerprint RECOMPUTED from the supplied
public key (mismatch ⇒ typed `FingerprintKeyMismatch`) → typed
`TrustedHostKey` record bound (host, port, key type, key, fp, time,
provenance) → AetherCore-owned `trusted_keys.json` + `known_hosts`
regenerated exclusively from records → `StrictHostKeyChecking=yes`. No
TOFU; `~/.ssh` untouched; private-key bodies typed-rejected
(`PrivateKeyMaterialRejected`); unsupported key types rejected; malformed
base64 rejected. `fingerprint A + public key B` cannot combine.

## 2. Typed AuthReference persistence (corrective C)

`upsert_fleet_host` parses through the authoritative strict
`FleetHost::from_bytes` (`deny_unknown_fields` + full re-validation) and
derives DB columns via an EXHAUSTIVE `match` on `AuthReference` — no
`unwrap_or("agent")` fallback. Read-back restores the same typed domain;
corrupt `auth_ref_kind` combinations are hard read-time rejections.
CF-2 round-trips: Agent/KeyFile/Certificate restore exactly; malformed,
unknown-field and secret-shaped writes are rejected; path references are
never replaced by contents.

## 3. Scheduled compliance orchestration (corrective D)

`crates/fleet/src/scheduler_runner.rs` executes the full pipeline:
schedule → due evaluation → scope resolution → bounded orchestrator →
remote read-only compliance → per-host results → history append →
`last_result` → `next_run` advance. Injectable `SchedulerStore` + fake
transport prove the COMPLETE pipeline hermetically. CLI:
`aetherctl fleet schedule run-due`. Only due+enabled schedules run;
single-run overlap lock; missed runs catch up exactly once; cancellation
propagates; schedule failures isolated; deterministic ordering.

## 4. Fleet desktop UX (corrective E)

FleetPage is now a full management surface: host list, add host, edit
non-secret metadata, remove with explicit two-step confirmation,
trust/untrust flow (public key + fingerprint BOTH required; the backend
rebinds and re-verifies), fingerprint/key status, enable/disable. Trust,
probe, audit and compliance stay read-only and typed; statuses
(NotVerified/NotAvailable/HostKeyMismatch/Timeout/AuthFailure/
Incompatible) are documented contract states in
`docs/phase34/REMOTE_OPERATIONS.md`. EN/AR parity (56/56 fleet keys),
RTL-safe, no secret rendering, additive typed Tauri commands only.

## 5. Reconstruction counts (corrective G)

`scripts/_p34_reconstruction_test.py` performs two independent P33 → P34
reconstruction cycles and emits the machine-readable artifact
`/tmp/p34_reconstruction_result.json`. Its required fields prove both
equations: `ledger_equals_comparison` and
`raw_equals_scoped_plus_self_excluded`, with an explicit sorted
`self_excluded_paths` list and zero `mismatch_count`. The scoped count is
the authoritative full-tree count; build/dependency/runtime caches and the
patch-delivery directory are intentionally outside that scope.

## 6. Compatibility handshake (corrective F)

`compatibility_verdict` requires the product envelope AND the remote
contract marker (`remoteContract == fleet.remote.v1`); missing/wrong
marker or malformed envelope ⇒ typed Incompatible — semver alone never
authorizes. CF-5 fixtures use the exact real command-output wire shape.

## Acceptance gates (re-run after all corrective bytes)

- Cargo ×2: `cargo test --workspace --jobs 2` — 121 suites, 547 passed,
  0 failed, 0 ignored (provisional baseline 510 → +37), runs equivalent.
- P34 adversarial audit: strict superset of the inherited 846 checks with
  individually named trust, scheduler, Desktop Fleet B1–B6, reconstruction,
  and SSH-safety gates; failures=[] PASS.
- Clippy: `cargo clippy --workspace --all-targets` exit 0.
- Svelte: 0 errors / 17 warnings (inherited baseline unchanged).

## Honesty statements

- LIVE_REMOTE_SSH=NotAvailable — no disposable SSH test target existed on
  the proof host; none was fabricated. Deterministic hermetic proofs cover
  transport semantics; live round-trip qualification remains QD-034-003.
- Windows-native fleet/scheduler qualification remains QD-034-001/002/006.

Authoritative archive hash: see PHASE34_FINAL_SHA256.txt
