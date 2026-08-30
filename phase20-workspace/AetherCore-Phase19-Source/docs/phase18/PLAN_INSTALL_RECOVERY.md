# Immutable Plan, Installation, Recovery, and Verification

The existing one-shot consent and operation-engine plan remain authoritative. `DriverInstallAction` now seals target version, authority type/provider, official source, recommendation state, installation mode, trust state, and a provenance digest in addition to the exact WUA update ID/revision and device evidence.

Before Windows-managed install, `driver-install` revalidates the sealed provenance digest, device presence/class/matched ID/current driver/problem state, authority type, installation capability, trust state, recommendation state, and existing exact WUA identity. Candidate/package substitution fails closed.

Recovery evidence is prepared before mutation through the existing driver backup, restore-point and install-journal architecture. Batch continuation is conservative: failure after mutation or recovery-required halts following work; failure before mutation may allow independent work; reboot is aggregated unless dependency/provider semantics require a boundary.

Completion is evidence-based: post-install status records before/after version and problem code, result, reboot, backup root, and verification. A process exit code alone is not sufficient evidence.
