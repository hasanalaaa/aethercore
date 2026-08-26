# EXTENSIBILITY.md — The Future-Proofing Contract

How to extend AetherCore without forking its guarantees. Each recipe lists the exact
trait/table/audit-gate to touch. Anything added this way inherits the audit chain
automatically.

---

## Recipe 1 — Add a telemetry provider (new OS / new counters)

1. Implement the trait in `crates/performance-telemetry/src/<your>_impl.rs`:
   `impl PerfPlatform for YourPerfPlatform`. Keep the Phase 20 disciplines:
   MIN_INTERVAL_MS clamp, `.normalized()` on every snapshot, CollectorFault isolation.
2. Register in `default_platform()` (`src/lib.rs`) under a `#[cfg(...)]` arm — never
   behind runtime detection.
3. Tests (mirror `tests/native_providers.rs`): tick-delta math vs injected counters,
   hostile values clamp, every fallible sub-collector degrades to CollectorFault,
   live integration if the host supports it.
4. Capability truth-upgrade: flip the matrix row in `crates/platform-capabilities`
   ONLY with code+test backing (audit gates `p27-honesty-native-code-backed:*`).
5. Audit: rerun `scripts/phase30-adversarial-audit.py` — honesty + conformance gates
   apply to your file automatically via the src glob.

Reference implementation: `src/macos_impl.rs` (libc-only), `src/linux_impl.rs` (/proc).

## Recipe 2 — Add a diagnostics domain (new engine / new file type)

1. New module in `crates/db-diagnostics/src/`. Open targets READ-ONLY; set explicit
   capacity bounds (MAX_* consts). Never open read-write, never connect implicitly —
   declare the lane NotAvailable instead.
2. Emit findings only through `DbFinding::try_new` with a complete EvidenceRef matrix
   (empty evidence ⇒ no finding, by construction).
3. Seal with `model::seal_report` for the deterministic digest.
4. Fixture tests in `crates/db-diagnostics/tests/fixtures.rs`: risky → exact findings;
   clean → zero; malformed → skipped typed; determinism ×2 byte-equal.
5. Audit: forbidden-verb scan and deps allowlist cover your module automatically
   (`p30-forbidden-verb:*`, `p30-deps-allowlist`). Add new deps ONLY with an
   owner-review waiver comment in that crate's Cargo.toml.

## Recipe 3 — Add an aetherctl command

1. Parse: add the verb in `apps/aetherctl/src/cli.rs` (`match name.as_str()`), flags
   through the Cursor helpers (`parse_u32`, `parse_i64`, `value_after`). Unknown flags
   must fail typed (`unknown_flag`).
2. Route: Offline → `OfflineJob` + `offline::execute`; daemon-backed → `ServiceJob` +
   `service_cmds::execute` calling the SAME router payload the desktop uses (CX-3).
3. Render strictly through `render::finish` / the versioned envelope constructor —
   raw `serde_json::json!` outside the envelope path is audit-banned.
4. i18n: add EN+AR keys in `apps/aetherctl/src/i18n.rs`; parity is gate-checked
   (`p31-cli-i18n-parity`).
5. Registry: add one row to the CLI↔router parity table (see
   docs/phase28/ROUTING_TABLE.md); the phase28 parity gate asserts 1:1 coverage.

## Recipe 4 — Add an insight evidence surface

1. Produce evidence rows from your domain as typed records (see how db-diagnostics
   emits findings; how performance-bottleneck emits `Finding`s).
2. Map into the intelligence surface's EvidenceInput in
   `services/maintenance-service/src/intelligence.rs` — reuse the existing citation
   resolver so renderer insights link back to facts.
3. Feed timeline events through `streaming::publish_*` (EventBus sequence space) so
   hydration/replay stays race-free — never a side channel.
4. Tests: fixture → expected citations; hostile input → dropped silently by the rule
   (no hollow insight).
5. Audit: mutation-guard scan covers your call sites automatically; keep writes out of
   read paths (`p30-forbidden-verb:*` pattern).

## Recipe 5 — Add a capability-matrix row

1. Append the enum variant in `crates/platform-capabilities/src/lib.rs` (`Capability`).
2. Add exactly one row per platform table (`windows_table` is generated from ALL;
   macOS/Linux tables cite honest Availability with reason keys from the closed
   vocabulary `keys` module).
3. If claiming Native: implement + prove it first (Recipe 1/2) — the audits
   `p27-honesty-native-code-backed:<platform>:<cap>` grep-verify code backing.
4. Surface: renderer About panel picks the row up automatically; add EN+AR strings for
   any new reason key (i18n parity gate).
5. Audit: `p26-matrix-row-coverage:*` asserts full coverage on every platform.

---

## Invariants that make this safe

- Wire tags are ADDITIVE ONLY with freeze lists asserted by every phase audit.
- Every untrusted-input parser gets a fuzz target under `fuzz/` (see
  .github/workflows/fuzz.yml).
- Byte-level tree integrity: PHASE_3x_BINARY_SAFE_PATCH hash manifests +
  PHASE_3x_EXPECTED_FULL_SHA256.json dual-mode verifiers.
- Consolidated debt: DEBT_REGISTER.json (append-only; closures cite evidence).
