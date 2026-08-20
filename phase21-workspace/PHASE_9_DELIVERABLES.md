# Phase 9 Deliverables — Security & Release Closure

## Milestone outcome

Phase 9 converts Phase 8's local privileged boundary into a principal-owned, one-shot-consent production contract and closes the remaining cleanup file-identity race. It also makes dependency approval and release verification fail-closed and removes prototype operation/authorization surfaces.

## 1. Session & principal ownership

- `crates/security`: `PrincipalContext` binds the exact named-pipe connection by impersonating the client and reading its thread token for user SID, token logon `AuthenticationId`, and elevation; the pipe supplies Windows session ID. PID is retained only to resolve broker image identity. `RevertToSelf` is mandatory before request dispatch, and `binding_key()` is internal service authority.
- `services/maintenance-service`: derives the principal before dispatch and never accepts caller-supplied ownership.
- `operation-engine` + SQLite: every immutable plan persists `owner_principal_key`; plan snapshot, event counts, execution/status/history and mutation entry paths are owner-scoped.
- Driver Hub, system-repair assessment, cleaner snapshot, startup snapshot, and diagnostic snapshots enforce owner isolation for in-memory/readback evidence.

## 2. Authorization 2.0

- Protocol v7 retires prototype demo/challenge/grant payloads.
- Non-elevated desktop calls `BeginConsentIntent`, then launches the fixed installed broker with only `--intent-id <uuid>`.
- Elevated broker fetches the trusted service plan summary before presenting approval.
- Service validates elevated broker identity and same principal binding.
- An approved intent cannot be presented or approved again.
- `consume_consent_and_transition` atomically consumes one approved/unexpired intent and moves exactly its owned immutable plan to `Preflight`; failure rolls the transaction back.
- Driver/repair/cleanup/startup start paths validate ownership before any mutation side effect. If post-consent execution-journal initialization fails, the process-local mutation slot is released and the plan is failed closed instead of being stranded in `Preflight`.

## 3. Exact cleanup file identity

Cleanup evidence now includes volume serial and 128-bit file ID collected with `GetFileInformationByHandleEx(FileIdInfo)`. The mutation path reopens safely, revalidates final-path/evidence plus exact file identity, and performs deletion through `SetFileInformationByHandle`. A regression test replaces a target with a same-path/same-shape file and requires rejection.

## 4. Dependency & build freeze

`scripts/freeze-dependencies.ps1` now binds:

- `Cargo.lock`
- `pnpm-lock.yaml`
- root and workspace dependency manifests, including `package.json`, all Cargo manifests, `pnpm-workspace.yaml`, `.config/dotnet-tools.json`, `.cargo/config.toml`, `deny.toml`, and `nuget.config`;
- `release/dependency-locks.sha256`;
- `release/dependency-manifests.sha256`;
- `release/dependency-freeze.json`, whose recorded lock/baseline hashes and Rust/pnpm pins are verified rather than merely checked for existence.

`-Refresh` is an explicit trusted-workstation action. `-VerifyOnly` is the CI/release behavior. CI no longer seeds an approval baseline automatically.

### Authoring-environment limitation

This delivery environment has no Rust toolchain/PowerShell and cannot reach the package registries needed to resolve the missing Phase 8 lockfiles. Therefore no lockfile was fabricated. The source remains deliberately **pre-freeze** until the trusted Windows freeze workstation runs `freeze-dependencies.ps1 -Refresh`; `verify-phase9.ps1` fails closed until that evidence exists.

## 5. Production sanitation

- Removed Phase 1 demo operation endpoints/actions.
- Removed challenge/grant authorization payloads and desktop command semantics.
- Renamed UI layering files from phase archaeology (`legacy.css`, `phase7.css`) to production roles (`foundation.css`, `luxury.css`).
- Current product documentation describes consent intents rather than retired grants.

## 6. Verification

- `scripts/phase9-security-audit.ps1`: principal derivation, ownership persistence, consent-intent closure, exact file identity, and retired-surface rejection.
- `scripts/verify-phase9.ps1`: inherits all Phase 0–8 checks, verifies the approved dependency freeze, runs Phase 9 security tests, checks the locked workspace/UI, and can inherit installer/signing/privilege checks.
- `scripts/static_validate.py`: Phase 0–9 source gate; current authoring result is 114/114 checks passed.

## GA gate

A signed public release still requires executing `verify-phase9.ps1` on supported Windows with the committed dependency freeze. Installer lifecycle validation must run on a disposable VM; the non-elevated desktop token check must run from a normal installed user session.
