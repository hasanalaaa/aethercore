# Secure Update and Cryptographic Support Export

Phase 15 adds two deliberately separate capabilities: verified application update orchestration and privacy-first diagnostic export. Neither capability weakens the Phase 9–14 privilege, ownership, mutation, IPC, or scheduler boundaries.

## Update trust boundaries

AetherCore has one installer authority: the WiX MSI/Burn pipeline. The in-app updater never introduces a second package manager and never asks the LocalSystem service to execute an arbitrary path or command.

The update path is split into three authorities:

1. **User-scope downloader.** `aethercore-desktop` links `aethercore-update-download` and is the only production component that performs update HTTPS requests. It obtains manifest/signature/package URLs from a service descriptor; Svelte never supplies an URL. Redirects are disabled, connect/overall timeouts are finite, bodies are bounded, and package bytes are streamed to `%LOCALAPPDATA%\AetherCore\UpdateCache`.
2. **LocalSystem verification and staging authority.** The maintenance service owns `update-trust.json`, verifies the Ed25519 signature over the exact manifest bytes before JSON parsing, enforces channel/key/time/package constraints and the durable rollback/equivocation floor, then issues a bounded upload descriptor. The desktop uploads bytes in <=256 KiB sequential chunks; no desktop filesystem path is accepted. The service checks exact size and SHA-256, validates Authenticode, checks SHA-256 again, then renames into a service-derived staging path.
3. **Fixed elevated execution authority.** Only `aethercore-update-broker.exe` at the MSI-owned installation path and with an elevated token may read/claim/complete an install intent. It receives only `--intent-id` and allowlisted `--locale`, rechecks exact hash/size, validates Authenticode, rechecks hash, and launches the service-issued Burn path with no arbitrary arguments.

Installation claims the machine-wide **`MutationSupervisor::Update`** lease. Driver installation, repair, cleanup, startup mutation, and update execution therefore cannot overlap. The elevated broker also takes the existing cross-process `Global\\AetherCore.WindowsUpdateMutation.v1` mutex immediately before claim, preserving mutual exclusion across service/process restart boundaries. The mutex is defense-in-depth; `MutationSupervisor::Update` remains the logical authority. A restart-safe singleton ledger record preserves the already-verified staged artifact identity and lease ownership, but stores no download URL, command line, consent secret, or arbitrary executable authority. The guard persists release ID, exact version, stable/beta channel, notes message key, minimum Windows build, staged path, size/hash, ticket and expiry so service restart reconstructs an `Installing` snapshot with the same non-actionable release identity rather than presenting `Idle`.

Install-intent lifecycle is one-shot and race-safe. Claiming an intent reserves it atomically under the coordinator intent lock **before** hash/Authenticode verification or mutation-lease acquisition, so a concurrent cancellation cannot revoke consent after elevated claiming has begun. If that pre-execution claim fails, the reservation is released; once the durable execution guard exists, cancellation no longer owns the operation. Staging failures are terminally reflected as `Failed` rather than leaving the snapshot stranded in `Staging`. User UAC decline and unclaimed-intent expiry restore the snapshot to `Staged` (or the best earlier non-install state) instead of leaving `AwaitingConsent` stuck. Only one unclaimed install intent is retained per owner. Execution-expiry cleanup is fail-closed: the in-memory Update lease is not released unless deletion of the durable execution guard succeeds, so a transient SQLite failure cannot make a possibly-active installer look free for another mutation.

### Signed static manifest

`update-trust.json` pins channel-specific Ed25519 public keys and credential-free HTTPS manifest/signature URLs. A release manifest contains a monotonic sequence, generation/expiry times, release identity, minimum Windows build, Burn URL, exact byte size, and SHA-256. The manifest envelope signs the exact bytes that are fetched. The SQLite manifest floor records both the highest sequence and manifest SHA-256 so same-sequence equivocation is rejected as well as ordinary rollback. Candidate selection also rejects a release whose signed `minimum_windows_build` exceeds the actual host build returned by `RtlGetVersion`; the WiX/Burn launch condition remains a final installer-side backstop rather than the first compatibility check.

Authenticode is a second validation layer over the already manifest-bound artifact. Hash verification is repeated around Authenticode at service staging, at one-shot install claim/restart recovery, and again in the elevated broker immediately before execution. The signed manifest hash remains the content identity; Authenticode adds Windows publisher-chain validation rather than replacing manifest trust.

Production signing is fail-closed: `-RequireSigning` also requires an enabled, externally provisioned update trust configuration. `release/update-trust.template.json` is intentionally disabled and contains no fake release key.

## Support export trust and privacy boundaries

Support export is an allowlist builder, not a generic log zipper. The service may construct only selected product metadata, sanitized diagnostics, owner-scoped journal history, and owner-scoped scheduler activity. It does not export raw minidump bytes or raw Event Log XML.

The user must first request an inspection preview. Sanitization is completed before that preview, which exposes included sections, estimated size, and redaction counts. The current sanitizer masks personal profile paths, account identifiers, SIDs, email identifiers, and hardware serials, including SIDs and email addresses embedded inside otherwise free-form diagnostic strings. Hardware serials are replaced with a non-linkable literal redaction rather than a pseudonymous hardware hash. The engine also enforces bounded retained-object quotas (eight previews and eight prepared bundles globally, with at most one active prepared bundle per owner) so preview/export requests cannot become an unbounded service-memory or staging-disk sink.

After explicit export, the service creates a deterministic USTAR archive with sorted allowlisted payloads. Each payload has exact size and SHA-256 in `bundle-manifest.json`; the manifest also commits to the ordered payload root hash. An installation-local Ed25519 key signs the manifest. The proof claim is intentionally narrow: it is **tamper-evident output generated by this AetherCore installation, not vendor or hardware attestation**.

Archive parsing is bounded and validates TAR header checksums before accepting an entry. Installation-key creation is serialized so concurrent exports cannot race key initialization.

The public key embedded in the archive is not accepted as its own root of trust. `SupportBundleReady` returns the installation public-key SHA-256 fingerprint over authenticated IPC. The strong verifier requires that independently obtained fingerprint and rejects a bundle whose embedded key differs, even if an attacker re-signs modified contents with a new key. It also rejects duplicate/unsafe paths, unmanifested archive entries, malformed TAR structure, size/hash/root mismatches, altered claim/key kind, and signature failures.

The LocalSystem service never receives an export destination path. It only exposes bounded owner-scoped chunks. The non-elevated desktop writes the final `.aetherdiag` into the user's Downloads folder, never overwrites an existing file, recomputes the complete archive SHA-256 while copying, validates the installation public-key fingerprint, and re-runs the strong archive verifier (`verify_strict` for the Ed25519 proof plus TAR/manifest/hash/root validation) against the independently received installation-key fingerprint **before the final rename**. On both success and failure the desktop discards the service-side prepared bundle, and failure also removes the temporary user file, so retained-object quotas are released deterministically. `aethercore-support-bundle-verify` provides an offline verification path when the expected fingerprint is supplied independently.

## IPC and observability

`update.proto` and `support_bundle.proto` carry typed contracts. Update snapshots and support-bundle lifecycle events travel over the existing authenticated principal-scoped IPC v7 event stream. Update URLs are never UI inputs to privileged endpoints; support destination paths are never service inputs. All known update/support failures map to localized message keys.

## Release gates

`verify-phase15.ps1` runs Phase 0–14 first and deliberately defers release packaging. It then executes the Phase 15 architecture audit, full workspace compile, signed-manifest/upload/mutation/tamper tests, UI type/build checks, localization parity, and aggregate static invariants. Only after those pass may reproducibility, WiX packaging, signing, and installer lifecycle verification run.

Windows-native GA qualification still requires the disposable-machine lifecycle gate, live Authenticode behavior, UAC broker execution, real HTTPS endpoints, service restart during update, and update rollback/failure scenarios.

## Zenith note

Zenith does not modify update manifest trust, staging identity, Authenticode validation, installer authority, support-bundle redaction, archive integrity, or installation-local trust identity. The System Care progress surface now reuses the shared semantic/compositor-friendly progress primitive; this is a renderer-only presentation change and does not infer progress that the native update engine did not report.
