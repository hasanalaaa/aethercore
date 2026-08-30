# The local-only guarantee

**After installation, AetherCore works with no internet, no server, no account,
and no external service.** Every diagnostic, repair, cleanup, optimization,
timeline, care, security-audit, compliance and intelligence feature runs against
the local machine and the embedded model. Nothing is required to be reachable.

There is no account. There is no licence server. There is no telemetry.

This document states the guarantee, names the only three code paths that can open
a socket, and shows how each is user-initiated and off by default.

---

## 1. What is enforced

`crates/intelligence-core/tests/offline_boundary.rs` — one test, no framework, no
new dependency. It resolves the whole workspace dependency graph via
`cargo metadata --offline` and fails if any crate outside the allowlist acquires a
network-capable dependency, directly or transitively.

Run it:

```bash
cargo test -p aethercore-intelligence-core --test offline_boundary
```

Current state: **47 of 50 workspace crates have no network-capable dependency
anywhere in their graph.**

## 2. The three permitted network paths

| Crate | What it does | How it starts | Default |
|---|---|---|---|
| `aethercore-update-download` | HTTPS GET of an update manifest, its detached signature, and the package | `aetherctl update download`, or the desktop "Check for updates" action | **Off.** `UpdateCoordinator` reads `update-trust.json` from the product directory. Absent → `enabled: false, channels: []`. `check_descriptor` returns `UpdateEngineError::Disabled` before a URL is even constructed |
| `aethercore-driver-acquisition` | HTTPS GET of a driver package from a vendor host, redirect policy `none`, provider-pinned | The user picks a driver from the locally-built catalogue and asks to download it | **Off.** No background scan, no automatic fetch |
| `aethercore-desktop` | No network call of its own — links `update-download` so the UI can offer the update action | n/a | Inherits the above |

These are the only two files in the product that call an HTTP client:
`crates/update-download/src/lib.rs` and `crates/driver-acquisition/src/lib.rs`.

Removing them is **not** the goal. Making offline the guaranteed default is.

### Evidence that update trust ships disabled

`crates/update-engine/src/coordinator.rs`, when `update-trust.json` is absent:

```rust
UpdateTrustConfig {
    schema: manifest::TRUST_SCHEMA.into(),
    enabled: false,
    channels: Vec::new(),
}
```

and every check is gated:

```rust
if !self.trust.enabled {
    return Err(UpdateEngineError::Disabled);
}
```

The file the installer actually places, read back from a real installed machine
(`C:\Program Files\AetherCore\update-trust.json`, Windows 11 ARM64):

```json
{
  "schema": "aethercore.update-trust.v1",
  "enabled": false,
  "channels": []
}
```

`enabled: false`, **0 channels**. In-app update is inert until an operator
deliberately supplies a trust configuration with a signing key.

## 3. The intelligence core is offline by construction

`aethercore-intelligence-core` has four dependencies: `serde`, `serde_json`,
`sha2`, `thiserror`, plus `llama-cpp-2` (a build-time C++ binding that performs no
network I/O). The enforcement test proves no network-capable crate reaches it.

The model is embedded, resolved from a pinned relative path, and **hash-verified
before it is loaded** (`crates/intelligence-core/src/llama.rs`):

```rust
pub const EMBEDDED_MODEL_RELATIVE_PATH: &str =
    "assets/models/qwen2.5-1.5b-instruct-q4_k_m.gguf";

pub fn activate_embedded_reasoner(product_root: &Path) -> Result<String, String> {
    let model_path = product_root.join(EMBEDDED_MODEL_RELATIVE_PATH);
    let pinned = embedded_model_entry();
    verify_model_hash(&model_path, &pinned)?;   // <- before load
    let mut reasoner = LlamaCppReasoner::new();
    reasoner.load(&model_path)?;
    ...
}
```

`verify_model_hash` refuses on mismatch:

```rust
if actual != pinned.sha256_hex.to_lowercase() {
    return Err("model hash mismatch: refusing to load".into());
}
```

The expected digest is compiled in — a swapped manifest on disk alone cannot
weaken verification. Verified against the shipped artifact:

```
pinned in source: 6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e
sha256 on disk:   6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e
```

The model is enabled by default (Phase 23.1). There is no download step and no
manual placement step.

## 4. No analytics, no crash reporting, no phone-home

- No analytics or crash-reporting SDK appears anywhere in `Cargo.lock`: no
  sentry, bugsnag, rollbar, datadog, opentelemetry, appinsights, segment,
  amplitude, mixpanel, posthog, firebase, crashlytics, raygun, honeycomb.
- The only HTTP client in the tree is `reqwest`, reachable from exactly the three
  crates in §2.
- The only URL literals in product source are: test placeholders under the
  reserved `.invalid` TLD, and three vendor **landing pages**
  (nvidia.com, intel.com, amd.com) held as display strings in
  `crates/driver-authority/src/truth.rs` and `crates/gpu-policy/src/lib.rs`.
  They are shown to the user; they are never fetched.
- `hardware-telemetry` and `performance-telemetry` read **local sensors** —
  SMART, NVMe health, PDH counters, memory pressure. The data never leaves the
  machine. The word "telemetry" here means hardware instrumentation, not
  reporting.

## 5. What happens with networking unavailable

| Area | Offline |
|---|---|
| Diagnostics, repair, cleanup, startup management | Works |
| Performance sampling, bottleneck attribution, optimization | Works |
| Timeline intelligence, recurrence, care orchestration | Works |
| Security audit, CIS L1/L2 compliance, signed report generation and verification | Works |
| Local intelligence / insights (embedded model) | Works |
| Driver catalogue, backup, restore points, PnP inspection | Works |
| Update *verification* of an already-present manifest/package | Works (offline signature verification) |
| Update *check / download* | Requires network **and** an operator-supplied trust config. Disabled by default |
| Driver *download* | Requires network. User-initiated per driver |

Nothing degrades because a server is unreachable — there is no server to reach.

## 6. Rules for future work

1. Do not add a network-capable dependency to any crate not in the allowlist.
   The enforcement test will fail; that failure is the design working.
2. Adding a crate to `NETWORK_ALLOWED` is a **product decision**, not a build fix.
   It requires a row in §2 naming the explicit user action that starts it and why
   it is off by default.
3. No feature may become network-required. A network path may only ever be an
   opt-in addition to a working local path.
4. No telemetry, analytics, or crash reporting. Ever — including for billing
   (see `docs/adr/ADR-LICENSING.md`).
