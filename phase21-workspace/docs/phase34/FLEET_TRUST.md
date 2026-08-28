# Phase 34 — Fleet Trust Model (corrective closure)

## Trust chain (end-to-end)

```
UNTRUSTED KEY CANDIDATE
        ↓  public key material (base64 blob, PUBLIC only)
fingerprint computed (SHA-256, lowercase hex)
        ↓
explicit user authorization (UI trust form / CLI trust action)
        ↓
fingerprint RECOMPUTED from the supplied public key
        ↓  recomputed == authorized ?  → else typed FingerprintKeyMismatch
public host-key material + fingerprint bound together
        ↓  (typed TrustedHostKey record: host, port, key type, key, fp, time, provenance)
AetherCore-owned trust store
        ↓  known_hosts REGENERATED from stored records only
        ↓  (companion trusted_keys.json, 0600 on Unix)
StrictHostKeyChecking=yes
        ↓
SSH operation (BatchMode, no prompts, read-only commands)
```

A SHA-256 fingerprint ALONE can never produce a known_hosts line or enable
remote work: every persistence path re-verifies the fingerprint against the
key material, so `fingerprint A + public key B` cannot be combined, and
private key bodies (`-----BEGIN ... PRIVATE KEY-----`) are structurally
rejected at the trust boundary (`PrivateKeyMaterialRejected`).

## States

| State | Meaning | Remote work |
|---|---|---|
| `NotVerified` | No pin recorded | Refused — host is never contacted |
| `Trusted` | Pin recorded, matching public key in trust store, presented fingerprint matches | Allowed |
| `HostKeyMismatch` | Pin exists, presented key differs | Blocked (hard) |

## Pin lifecycle

1. **Discovery (optional, untrusted).** If key discovery is performed it may
   present an untrusted candidate fingerprint AND the candidate public key
   material. Discovery never creates trust and never persists a record.
2. **Explicit user authorization.** `aetherctl fleet trust --id <hostId>
   --fingerprint <sha256> [--key-type <t>]` records the pin after the user
   has verified the fingerprint out of band. The desktop trust flow requires
   BOTH the public key (base64) and the fingerprint. The only trust-creation
   constructor is `TrustedHostKey::authorize`, which recomputes the
   fingerprint from the key and rejects mismatches, unsupported key types
   (`ssh-dss` etc.), malformed base64, and private-key bodies.
3. **Persistence.** The typed record is stored in
   `<state-dir>/fleet/trust/trusted_keys.json`; the AetherCore-owned
   `known_hosts` is regenerated exclusively from those records (one
   canonical `[host]:port key-type base64-key` line per host). The same
   fingerprint is bound into the inventory pin via `FleetHost::pin_trust`.
4. **Verification.** At every remote operation the trust decision is
   recomputed from the pin; the OS ssh client additionally enforces
   host-key checking against the owned `known_hosts`.
5. **Revocation.** `fleet untrust` / the UI "revoke trust" action removes
   BOTH the record and the known_hosts line. Trust replacement always
   requires a new explicit user action; the old key line is removed on
   re-trust.

## Store isolation

- AetherCore-owned directory: `<state-dir>/fleet/trust/` holding
  `known_hosts` + `trusted_keys.json` (mode 0600 on Unix).
- The user's global `~/.ssh/known_hosts` is never read, modified, or
  referenced. No private key material can enter the store (typed rejection).
- No `GlobalKnownHostsFile` overrides are emitted; only the owned file plus
  strict checking.

## Changed-host-key response

A fingerprint mismatch is a typed `HostKeyMismatch` result both in the
orchestration layer and (defense in depth) as an ssh-level
"Host key verification failed" classification. The operation is blocked; no
warning-and-continue path exists. Accepting a NEW key requires a new
explicit authorization bound to the new key material; re-trusting removes
the previous line.

## Proofs (CF-1)

`cf1_fingerprint_only_trust_cannot_create_execution_entry`,
`cf1_malformed_base64_rejected`, `cf1_unsupported_key_type_rejected`,
`cf1_fingerprint_key_mismatch_rejected`,
`cf1_private_key_material_never_enters_trust_storage`,
`cf1_valid_key_plus_fingerprint_accepted_line_is_exact`,
`cf1_changed_host_key_is_host_key_mismatch`,
`cf1_unknown_host_stays_not_verified` — in `crates/fleet/src/trust.rs`.
