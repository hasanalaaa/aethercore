# ADR 0018 — Final Production Qualification and GA Seal

**Status:** Accepted — Phase 16

## Decision

AetherCore will treat General Availability as an evidence-aggregation decision rather than as a build-success flag. Phase 16 adds no product mutation authority. It introduces a four-part qualification boundary: inherited source/native verification, multi-host Windows presentation/accessibility qualification, live stress/soak and resilience evidence, and signed installer/supply-chain qualification.

The authoritative source/native gate is `verify-phase16.ps1`; passing it does **not constitute GA**. The definitive GA gate is `verify-production.ps1`, which may create a seal only after it validates evidence against `release/ga-matrix.json`.

Live concurrency qualification uses a fixed `aethercore-ga-probe` executable built from the same IPC crate and v7 contracts as the product. It is read-only and limited to Ping/HydrateSession traffic, bounded persistent sessions, reconnect and replay continuity. Stress evidence records request failures and service private bytes, handles, and thread counts after warm-up. An extended soak must satisfy the release duration and leak thresholds.

Windows presentation qualification is explicitly sharded. A signed witness records required surfaces plus locale, direction, DPI, refresh, accessibility and input dimensions. Static checks are not accepted as substitutes for Narrator, high-contrast, physical scaling, mixed-direction text, or motion/transparency behavior.

Installer qualification uses the signed Burn bootstrapper for install and uninstall, MSI repair for deliberate ACL drift, and the installed-state verifier for service account/SID, ACLs, Authenticode and PE elevation levels. The update broker is explicitly included in the `requireAdministrator` boundary check.

The GA sealer hashes every accepted evidence file, commits the release SHA-256 inventory and evidence-manifest hash into `GA-SEAL.json`, then produces detached `GA-SEAL.p7s` CMS signed by the protected production certificate. The signer certificate is not sourced from renderer or release payload input. Final verification checks both commitments and the CMS signature.

## Consequences

- A single CI run cannot claim GA without the required Windows matrix and long-duration evidence.
- Evidence from multiple disposable VMs can be combined without exposing machine serials or user account identifiers in the seal.
- Stress regressions are reproducible through a real IPC client instead of synthetic unit-test-only traffic.
- Burn/MSI lifecycle, repair hardening, support/update crypto gates, and inherited Phase 0–15 controls remain independently testable.
- The GA seal is cryptographically bound to the exact release inventory and accepted qualification evidence.
- A platform-neutral authoring environment may produce Phase 16 source/package qualification, but Windows-native GA remains unclaimed until `verify-production.ps1` succeeds with real evidence.
