# Sigma Evidence Integrity and Release-Candidate Qualification

## Release-blocker closure

Sigma closes the Omega verification defect in which `scripts/omega-evidence.py` could mutate delivered source bytes through child audits and still return success after `MANIFEST.sha256` had become invalid.

The verifier now enforces these invariants:

1. `MANIFEST.sha256` is verified before any repository-owned audit executes. An invalid initial manifest fails closed and repository gates are not executed.
2. Every repository-owned Python gate is executed against a disposable clone of the delivered source tree. Generated reports are written only to the caller-selected evidence directory outside the source tree.
3. Each disposable audit clone is fingerprinted before and after execution. Any audit-side source mutation is recorded and fails source verification with `SIGMA-RB-002`.
4. The delivered source tree itself is fingerprinted before and after the complete run. Any byte change fails with `SIGMA-RB-001`.
5. The source manifest is verified again after all gates. Both pre-run and post-run manifest validity are included in the final `source_verification_pass` condition.
6. `scripts/static_validate.py` and the Phase 13-16 / Enterprise / Zenith audit scripts are read-only by default. Report materialization requires an explicit `--output` path.
7. Evidence output paths inside the source tree are rejected by `omega-evidence.py`.
8. Manifest entries must be canonical portable POSIX paths and source symlinks are forbidden; manifest regeneration refuses a symlink-bearing delivery instead of following it.

## Adversarial regression proof

Run:

```text
python scripts/sigma-evidence-integrity-test.py
```

The regression test operates only on disposable copies and proves five cases:

- the original `static_validate.py` offender is read-only by default and leaves a valid delivery byte-identical;
- a manifest-covered byte tamper cannot return exit code 0;
- an injected audit-side write is contained in the disposable clone and detected without modifying the delivered source; the normal Omega/Sigma qualification incorporates that mutation signal into `source_verification_pass`;
- a manifest path containing `..` cannot redirect verification to bytes outside the source root;
- source symlinks are rejected so manifest verification always binds to self-contained regular-file bytes.

## Evidence command

Evidence must be written outside the source tree, for example:

```text
python scripts/omega-evidence.py --output C:\AetherCoreEvidence\omega-evidence.json --blockers-output C:\AetherCoreEvidence\release-blockers.json
```

Use `--strict` for release-candidate qualification. Strict mode fails when source verification fails or when any release/environment blocker remains.

## Qualification boundary

Sigma makes the source verification system hermetic with respect to the delivered source bytes and closes the false-success manifest path. It does not convert non-Windows source evidence into Windows-native release proof. Release-candidate closure still requires the qualified Windows host, dependency-freeze approval, named-pipe teardown evidence, locked Rust/pnpm gates, service/token/SCM/WiX/AuthentiCode qualification, update/install lifecycle checks, and the existing Phase 16 production evidence/seal requirements.
