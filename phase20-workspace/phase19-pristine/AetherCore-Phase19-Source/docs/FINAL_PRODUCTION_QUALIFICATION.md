# Final Production Qualification and General Availability Seal

Phase 16 is the final qualification layer over the sealed Phase 0–15 product. It does not add a new privileged product feature. It adds release evidence, stress tooling, host-matrix qualification, lifecycle verification, and a cryptographic GA seal.

## Qualification model

AetherCore deliberately separates four evidence classes so one green build cannot impersonate full production qualification.

1. **Source/native qualification.** `scripts/verify-phase16.ps1` inherits every Phase 0–15 gate, runs the Phase 16 policy audit, compiles the full locked Rust workspace, builds the live IPC GA probe, and re-runs the aggregate static validator. Passing this gate alone **does not constitute GA**.
2. **Windows host qualification.** `scripts/phase16-host-qualification.ps1` validates a release on a concrete Windows build lane and commits a reviewed visual/accessibility witness. The combined evidence must cover the OS lanes, EN/LTR and AR/RTL, DPI/refresh-rate matrix, keyboard/Narrator/reduced-motion/reduced-transparency/high-contrast modes, mouse/keyboard/touch-or-pen input, and Stable/Beta update presentation.
3. **Stress, soak and adversarial resilience.** `phase16-stress-soak.ps1` drives the real persistent IPC v7 service with `aethercore-ga-probe`, reconnect/replay/hydration traffic, and process resource sampling. GA requires at least a 24-hour extended soak with zero request failures and bounded private-bytes, handle, and thread growth. `phase16-resilience-matrix.ps1` re-runs collector, scheduler, update/support tamper, IPC fuzz gates and repeatedly restarts the service while proving fresh reconnect/replay recovery.
4. **Installer and supply-chain qualification.** `phase16-installer-lifecycle.ps1` exercises signed Burn install, installed elevation/ACL checks, deliberate ACL drift followed by MSI repair, and Burn uninstall with ProgramData preservation. The release candidate must retain approved dependency freeze evidence, SBOM, reproducibility evidence, enabled update trust, SHA-256 inventory, and valid Authenticode signatures.

## GA matrix

`release/ga-matrix.json` is the machine-readable release contract. It defines the minimum Windows build inherited from packaging, three build-family lanes, accessibility/bidirectional/DPI/refresh/input coverage, the extended-soak requirement, and leak thresholds. `release/ga-witness.template.json` is the operator witness schema for visual and assistive-technology checks that cannot be honestly replaced by static source inspection.

A host evidence file is accepted only when every required surface and witness check is PASS. Host evidence records exact Windows build/UBR and the witness hash; it does not copy screenshots, account names, serial numbers, or other machine identifiers into the release seal.

## Live GA probe

`tools/ga-probe` uses the same `aethercore-ipc::SessionClient` used by production clients. It creates up to four principal-scoped persistent sessions, alternates Ping and HydrateSession requests, reconnects with replay sequence continuity, and rejects non-monotonic event delivery or non-200 service responses. It introduces no mutation authority and sends no arbitrary path, URL, command, or installer argument.

The soak runner selects its memory/handle/thread baseline only after warm-up. Thresholds are evaluated against the final process state and retained as JSON evidence. The thresholds are release gates, not health scores.

## Installer lifecycle

The Phase 16 lifecycle is intentionally performed only on an acknowledged disposable Windows VM. It uses the consumer Burn bootstrapper for install/uninstall, verifies the installed service/ACL/elevation boundary, deliberately adds a broad write ACE to the installation directory, repairs through MSI, verifies that hardening is restored, then uninstalls through Burn while confirming ProgramData preservation.

The installed-state verifier now checks all privileged PE boundaries, including `aethercore-update-broker.exe` as `requireAdministrator`, while the desktop remains `asInvoker` and the maintenance service remains LocalSystem with its unrestricted service-specific SID policy.

## Cryptographic GA seal

`verify-production.ps1` is the definitive release-seal gate. It calls `phase16-seal-release.ps1`, which refuses to seal unless the evidence directory contains:

- PASS evidence for every required OS lane and all aggregate UI/accessibility/bidirectional coverage;
- a PASS extended soak meeting the minimum duration and zero-failure policy;
- PASS adversarial resilience evidence with the configured restart floor;
- PASS Burn/MSI lifecycle evidence;
- valid release SHA-256 inventory, Authenticode signatures, SBOM, and production-enabled update trust.

The sealer creates `GA-EVIDENCE-SHA256SUMS.txt`, `GA-SEAL.json`, and a detached CMS signature `GA-SEAL.p7s` using the protected production signing certificate. The seal commits to the source commit, release SHA-256 inventory, complete evidence manifest, GA matrix, signer thumbprint, and the release claims. `verify-ga-seal.ps1` independently checks those commitments and the CMS signature.

The archive-embedded support-bundle key from Phase 15 is unrelated to this release signature. The Phase 15 support proof remains installation-scoped; the Phase 16 GA seal is a vendor release attestation.

## Authoritative commands

Source/native qualification:

```powershell
.\scripts\verify-phase16.ps1
```

Signed release candidate packaging:

```powershell
.\scripts\verify-phase16.ps1 -ReleasePackaging -RequireSigning -UpdateTrustPath <trusted-update-trust.json>
```

Per-host qualification is performed after installing the signed candidate and completing a witness:

```powershell
.\scripts\phase16-host-qualification.ps1 -Lane minimum-supported -WitnessPath .\witness.json -ReleaseRoot .\out\release\0.1.0 -StressProfile quick
```

The production signing runner creates the final seal only after all evidence is staged:

```powershell
.\scripts\verify-production.ps1 -ReleaseRoot .\out\release\0.1.0 -EvidenceDirectory .\out\ga-evidence
```

## Qualification boundary

Platform-neutral source audits can verify policy wiring, file integrity, schemas, and static invariants, but they cannot prove Windows-native SCM/UAC/WinVerifyTrust/WiX behavior, WebView2 rendering, real Narrator output, physical high-DPI display behavior, or long-duration resource stability. Those claims exist only when the corresponding Windows evidence has actually been produced and accepted by `verify-production.ps1`.

## Zenith pre-seal prerequisite

Post-Enterprise source changes are covered by `zenith-adversarial-audit.ps1`. The protected GA path now runs this audit before the existing Enterprise/Phase-16 policy and cryptographic seal checks. Release packaging remains governed by the existing signed Phase-16 boundary; Zenith does not create a second signing authority.
