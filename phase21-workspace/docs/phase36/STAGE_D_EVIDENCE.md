# Phase 36 — Stage D evidence: the update path

## D1 — `aetherctl update stage`: the full path — BLOCKED BY DESIGN, blocker named

**Windows does not qualify it, and no amount of Windows qualification will.**
The gate is not a runtime capability probe. It is an unconditional match arm in
the CLI's offline router, `apps/aetherctl/src/offline.rs:138`:

```rust
OfflineJob::UpdateCheck | OfflineJob::UpdatePlan | OfflineJob::UpdateDownload
| OfflineJob::UpdateStage | OfflineJob::UpdateStatus | OfflineJob::UpdateCancel
| OfflineJob::UpdateRollback
    => Err(CliError::capability_unavailable("updateApplyRequiresWindowsQualification")),
```

`apps/aetherctl/src/cli.rs:719-728` routes all seven subcommands to those
`OfflineJob` variants, so every one of them reaches that arm. There is no
platform test, no feature flag, no environment or trust-config input on that
path — the arm fires before any transport, filesystem or service call.

**Exactly what gates it:** a source change. Nothing observable about the
machine — not the ARM64 build, not the installed service, not the update-trust
configuration, not elevation — can route past a literal match arm. The
capability is not implemented for Windows in this revision; the typed
`CapabilityUnavailable` is the product deliberately failing closed rather than a
stub awaiting an environmental precondition.

Confirmed on the VM against the realigned install
(`evidence/D-update-path.txt`, aetherctl SHA-256 `ba2286f4…aac766`):

| command | result |
|---|---|
| `aetherctl update stage` | `capability not available (updateApplyRequiresWindowsQualification) [CapabilityUnavailable]` |
| `aetherctl update check` | same |
| `aetherctl update plan` | same |
| `aetherctl update download` | same |
| `aetherctl update status` | same |
| `aetherctl update cancel` | same |
| `aetherctl update rollback` | same |

What *is* implemented on the update surface, and is pure offline cryptographic
verification with no staging: `update verify --metadata --signature --keyring`,
`update offline verify <bundle.zip>`, `release inspect`, `release verify`.

## D2 — stage, apply, roll back — BLOCKED

Not reachable. `update stage` never produces a staged payload, so there is
nothing to apply and nothing to roll back. No restore point was needed and none
was consumed. This is a consequence of D1, not an independent failure.

## D3 — update trust disabled by default, no network required — PASS

**Zero channels, disabled.** The installed
`C:\Program Files\AetherCore\update-trust.json` reads, in full:

```json
{
  "schema": "aethercore.update-trust.v1",
  "enabled": false,
  "channels": []
}
```

83 bytes, SHA-256 `d4ad925d86f64560bd80c77eae8c606fe810c5f670a7cd42df0836b0653c8b37`
— byte-identical to `release/update-trust.template.json` in the repository, which
is the source `build-arm64-msi.cmd` stages into the payload. `Product.wxs`
installs it read-only under Program Files as `UpdateTrustComponent`, and no
private update key is installed by any component.

**No network access anywhere in this path.** Two independent proofs:

1. Structural — the `CapabilityUnavailable` arm in `offline.rs` returns before
   any transport is constructed. There is no code path from `update stage` to a
   socket in this revision.
2. Observed — all seven probes returned immediately with the typed error and no
   stdout. Nothing resolved a host, opened a connection, or timed out.

The install itself is also network-free: the MSI is a single self-contained
package with `MediaTemplate EmbedCab="yes"`, and the WebView2 bootstrapper —
the one component that *would* fetch from the network — belongs to the Burn
bundle (`Bundle.wxs`), which is not used here.

## GATE D — the update path's exact blocker is named with evidence.
