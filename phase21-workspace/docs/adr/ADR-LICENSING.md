# ADR-LICENSING — Constraints on a future paid tier

- **Status:** Accepted as a constraint record. **No code, no feature flags.**
- **Date:** 2026-08-30 (Phase 36 foundation session)
- **Scope:** What must remain true today so a paid tier stays possible tomorrow.

## Context

AetherCore is free. It may later require a paid licence. Nothing is being built
now. The only question this ADR answers is: **does anything we are building today
block a licence gate later, or force it into a shape that would break the product?**

The product's defining guarantee is `docs/LOCAL_ONLY.md`: after installation it
works with no internet, no server, no account, and no external service. That
guarantee is load-bearing — it is why the model is embedded, why update trust
ships disabled with zero channels, and why 47 of 50 crates have no network-capable
dependency.

A licence check is the classic reason products quietly break that guarantee.
This record exists to make that failure mode unavailable.

## Decision

### 1. A licence check MUST reuse the existing Phase 35 primitives

A future offline licence is **a signed file, verified locally**. It is the same
shape as an update manifest, and it must reuse the code that already verifies
those — not a parallel implementation.

Reuse, do not re-invent:

| Primitive | Location | Role for a licence |
|---|---|---|
| Ed25519 signing / verification | `crates/release-authority` — `sign_bytes`, `verify_signed_bytes` | Signs and verifies the licence file |
| `SignatureEnvelope` (`aethercore.release.signature.v1`) | `crates/release-authority` | The detached signature format |
| `TrustedKeyring` / `TrustedReleaseKey` (`aethercore.release.keyring.v1`) | `crates/release-authority` — `validate`, `active_key` | Holds the licensing public key; already supports `enabled`, `revoked`, `notBefore`, `notAfter` |
| `KeyRotation` (`aethercore.release.key-rotation.v1`) | `crates/release-authority` — `authorize` | Rotating the licensing key without shipping a new build |
| `canonical_json` | `crates/release-authority` | Deterministic bytes to sign — no signature ambiguity |
| Offline bundle verification | `apps/aetherctl/src/release.rs` — `verify_offline_bundle`, and `release verify` / `update verify` | Proven pattern: verify a signed artifact with **zero** network calls |

A licence schema would follow the same convention, e.g.
`aethercore.licence.v1`, carrying at minimum: schema, licence id, tier, issue
epoch, optional expiry epoch, and an optional machine binding. It is verified with
`verify_signed_bytes` against a keyring entry. **No server call. No account.**

The keyring already models expiry and revocation, so time-limited and revocable
licences need no new mechanism — only a revocation list delivered the same way the
licence is: as a signed file the user can be given by any means.

### 2. Where a licence gate would attach

At boundaries that already exist. A licence gate is an **authorization** decision,
and this codebase already has a governed authorization surface — it must not grow
a second one.

- **`services/maintenance-service/src/router.rs`** — the single governed RPC entry
  point every mutation already passes through. Every desktop and `aetherctl`
  action is already routed and trust-checked here. A tier check belongs beside the
  existing broker trust gates, returning a **typed** answer with a message key, in
  the same shape as every other refusal.
- **`crates/operation-engine` / `operation-kernel`** — consent and mutation
  authority. A paid-tier operation refused for licensing must be refused the same
  way a non-consented operation is refused today: typed, logged, no partial work.
- **`apps/aetherctl/src/exit.rs`** — the exit-code registry. A licence refusal gets
  a registered exit code, like every other typed failure.
- **i18n** — refusal text goes through the existing `en`/`ar` key parity gate.
  There is a test enforcing parity; a licence message is not exempt.

Reading the licence file belongs next to the update trust config in the product
directory — the same read-a-local-file-or-default pattern
(`crates/update-engine/src/coordinator.rs`), where *absent* is a valid, handled
state, not an error.

### 3. What must NOT be done

These are prohibitions, not preferences.

1. **No phone-home activation.** The licence must never be validated by contacting
   a server, at install, at start, or periodically. Verification is a local
   signature check. A product that cannot start on a plane is a different product.
2. **No forced account.** No sign-up, no login, no email, no device registration
   as a condition of use. The licence is a file the user possesses.
3. **No feature that only works online.** A paid feature must run offline like
   every free one. If a capability genuinely cannot exist without a network, it
   does not belong in this product.
4. **No telemetry introduced for billing.** Not usage counting, not seat counting,
   not "anonymous" metrics, not a heartbeat. `docs/LOCAL_ONLY.md` §4 forbids
   analytics and phone-home outright, and billing is not an exemption. If a
   business model needs measurement of the user's machine, change the business
   model.
5. **No network-capable dependency added for licensing.**
   `crates/intelligence-core/tests/offline_boundary.rs` will fail, and that
   failure is correct. Do not add the crate to `NETWORK_ALLOWED` to make licensing
   compile — that allowlist is for user-initiated update and driver download only.
6. **No online-only degradation.** An expired or absent licence must degrade to
   the free tier, never to a broken or nagging product.

### 4. Free tier stays fully functional offline

Explicitly, and permanently: **after a paid tier exists, the free tier must remain
fully functional with no internet, no server, no account, and no external
service.** The `docs/LOCAL_ONLY.md` guarantee applies to the free tier unchanged.

A paid tier may add capability. It may not subtract from what free users have
offline today, and it may not make the free tier depend on anything remote.

## Consequences

- Nothing built today blocks this. The signing, keyring, rotation, canonical-JSON
  and offline-verification primitives already exist and are qualified; a licence is
  another artifact through the same path.
- A licence gate is cheap **because** the governed router boundary already exists.
  Preserving that single boundary keeps it cheap. Bypassing it would not.
- Offline verification cannot revoke a licence already issued without delivering a
  signed revocation to that machine. That is the accepted cost of not phoning
  home, and it is the correct trade for this product.
- The keyring's `notAfter` gives time-limited licences without a server. Renewal is
  the delivery of a new signed file.

## Status of implementation

**None, deliberately.** No licensing code. No feature flags. No schema allocated.
No wire tag reserved. This record constrains a future decision; it does not start
one.
