# Phase 16 Deliverables — Final Production Qualification, Stress Matrix & GA Seal

## Master qualification

- `scripts/verify-phase16.ps1` — authoritative Phase 0–16 source/native master gate.
- `scripts/verify-production.ps1` — definitive GA release seal gate.
- `scripts/phase16-ga-audit.py/.ps1` — platform-neutral Phase 16 architecture/policy audit.
- `release/ga-matrix.json` — required Windows, bidi, DPI, refresh, accessibility, input, update-channel and stress coverage.
- `release/ga-witness.template.json` — human/assistive-technology witness contract for surfaces that cannot be proven statically.

## Stress and resilience

- `tools/ga-probe` — real persistent IPC v7 Ping/HydrateSession concurrency, reconnect and replay probe.
- `scripts/phase16-stress-soak.ps1` — warm-up baseline, prolonged live IPC traffic and private-bytes/handles/threads leak thresholds; `extended` is 24 hours.
- `scripts/phase16-resilience-matrix.ps1` — Phase 13 collector faults, Phase 14 scheduler/preemption faults, Phase 15 crypto/tamper regressions, IPC fuzzing and service restart/reconnect recovery.

## Windows GA host and installer evidence

- `scripts/phase16-host-qualification.ps1` — lane/build enforcement, full manual surface witness validation, installed signing/elevation/ACL checks, optional soak and resilience runs, JSON evidence output.
- `scripts/phase16-installer-lifecycle.ps1` — signed Burn install, installed security verification, ACL-drift repair, Burn uninstall and ProgramData preservation.
- `scripts/verify-installer-security.ps1` — extended with installed-state-only mode and explicit update-broker elevation verification.

## Supply-chain and seal

- `scripts/phase16-seal-release.ps1` — aggregate matrix enforcement, extended-soak/resilience/lifecycle requirements, SHA-256/AuthentiCode/SBOM/update-trust checks, evidence manifest and detached CMS GA signature.
- `scripts/verify-ga-seal.ps1` — independent release/evidence commitment and CMS signature verification.
- `GA-EVIDENCE-SHA256SUMS.txt`, `GA-SEAL.json`, `GA-SEAL.p7s` — generated only by the final Windows production seal gate.

## Documentation

- `docs/FINAL_PRODUCTION_QUALIFICATION.md`
- `docs/adr/0018-final-production-qualification-and-ga-seal.md`
- `PHASE_16_VALIDATION_SUMMARY.md`

Windows-native GA is intentionally not inferred from platform-neutral checks. It exists only after the signed Windows candidate, host matrix, 24-hour extended soak, resilience matrix, and installer lifecycle evidence have all been accepted by `verify-production.ps1`.
