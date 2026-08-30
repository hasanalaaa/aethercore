# ADR 0010 — Release Supply Chain and Reproducibility Boundary

## Status

Accepted — Phase 8.

## Decision

Require a reviewed dependency-freeze triplet (`Cargo.lock`, `pnpm-lock.yaml`, `release/dependency-locks.sha256`) for production release. Pin release/security tools, run dependency advisory/license/source gates, generate CycloneDX SBOMs, require deterministic Rust release settings, and sign artifacts only after build completion.

Do not claim byte-for-byte reproducible MSI output while the current WiX upstream nondeterminism issue remains unresolved.

## Rationale

Reproducibility is useful only when its boundaries are honest. Lockfiles, fixed tool versions, `SOURCE_DATE_EPOCH`, MSVC `/Brepro`, and native double-build checks make most of the pipeline auditable. Pretending the MSI itself is deterministic despite known varying package metadata would weaken rather than strengthen release evidence.

## Consequences

- First trusted connected Windows bootstrap must create and review the dependency freeze when the source seed lacks locks.
- Release workflow never auto-refreshes locks.
- MSI hash/signature identify the exact shipped package even though the package is not claimed reproducible byte-for-byte.
- If upstream WiX closes the reproducibility gap, this ADR should be revisited and the MSI claim can be tightened only after independent double-build proof.
