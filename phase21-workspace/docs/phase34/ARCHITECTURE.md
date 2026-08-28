# Phase 34 — Fleet & Secure Remote Operations — Architecture

## 1. Scope

Phase 34 extends AetherCore from a single-host platform to a fleet-aware one:
typed host inventory, secure SSH transport, fail-closed host-key trust,
bounded multi-host orchestration, read-only remote security/compliance
collection, deterministic scheduling with an executable tick path, a `fleet`
CLI surface, and a desktop Fleet management view (add/edit/remove/trust).

Security outranks convenience: there is no generic remote-shell feature, no
silent trust, no stored secrets, and no weakened SSH options anywhere.

## 2. New crate — `aethercore-fleet` (`crates/fleet`)

| Module | Responsibility |
|---|---|
| `domain.rs` | `FleetHost`, `FleetInventory`, `FleetSchedule`, trust pins, strict schemas (`aethercore.fleet.host.v1`, `aethercore.fleet.inventory.v1`, `aethercore.fleet.schedule.v1`) with `deny_unknown_fields` everywhere, fail-closed validation |
| `trust.rs` | Trust decisions (`Trusted` / `NotVerified` / `HostKeyMismatch`), AetherCore-owned trust store, safe argv builder, `shell_quote`, typed ssh exit classification |
| `transport.rs` | `SshTransport` over the OS OpenSSH client: typed argv (no shell), BatchMode, connect/operation timeouts, cancellation token, bounded capture, closed `RemoteOperation` set, compatibility handshake parser |
| `orchestrator.rs` | Bounded-concurrency batch runner (default 4, absolute ceiling 16), per-host panic isolation via `catch_unwind`, deterministic host-ordered results |
| `scheduler.rs` | Deterministic schedule semantics with injectable clock (`FixedClock`), structural 1-hour cadence floor, single-run overlap policy, catch-up-once missed-run behavior |

## 3. CLI — `aetherctl fleet`

Parsed in `apps/aetherctl/src/cli.rs` (`parse_fleet`) into typed `FleetJob`
values; executed in `apps/aetherctl/src/fleet.rs`. JSON output uses the
existing `aethercore.aetherctl.v1` envelope; text mode uses the existing
renderer. Inventory lives in the platform state dir
(`fleet/inventory.json`), schedules in `fleet/schedules.json`. Trust pins are
recorded in inventory and mirrorable into the AetherCore-owned trust store.

## 4. Persistence

Migration `0015_phase34_fleet` (additive, three tables:
`fleet_hosts`, `fleet_schedules`, `fleet_run_history`) plus additive
`Database` methods. Schema has no secret-shaped column. See
`SECURITY_MODEL.md`.

## 5. Desktop

`apps/desktop/src/main.rs` exposes typed Fleet commands for inventory CRUD,
trust/revocation, probe, security audit, compliance profile selection, and
schedule add/update/remove/run-due. Remote work is always routed through
`TrustedSshTransport`, which admits the host against the AetherCore-owned
trust store before spawning OpenSSH. `apps/ui` provides the Fleet page,
navigation entry, EN/AR catalogs, and explicit confirmation for destructive
actions; technical identifiers are isolated as LTR text and no secret
material crosses the boundary.

## 6. Wire contract

Unchanged. `operations.proto` request tags remain max 90, response tags max
52 (audit-gated `p34-wire-tag-append-only`). Fleet features deliberately do
not extend the v7 RPC surface: they are controller-local operations.

## 7. Documentation set

`SECURITY_MODEL.md` (threat model, secrets), `FLEET_TRUST.md` (pinning),
`REMOTE_OPERATIONS.md` (closed operation set, verification pipeline),
`SCHEDULING.md`, `SCORECARD.md`, `QUALIFICATION_DEBT.json`, `ISSUES.json`,
`PROGRESS.md`, and this document.

Authoritative archive hash: see PHASE34_FINAL_SHA256.txt

## Corrective closure modules

- `crates/fleet/src/scheduler_runner.rs` — the executable scheduler tick
  (`run_due_schedules`) connecting schedules → scope → orchestrator →
  history; injectable `SchedulerStore`/`Clock`/transport.
- `TrustedHostKey` + rebuilt `TrustStore` — typed trust records binding
  host/port/key-type/public-key/fingerprint; `known_hosts` regenerated from
  records only (fingerprint recomputed at authorize + persist time).
- Typed fleet persistence — `upsert_fleet_host` parses through the strict
  domain type; exhaustive `AuthReference` match; read-time corruption
  rejection.
- `compatibility_verdict` — remote contract marker check over the real
  command-output envelope; semver alone never authorizes.
- Desktop commands `fleet_add_host`, `fleet_edit_host`, `fleet_remove_host`,
  `fleet_trust_host`, `fleet_untrust_host`, `fleet_probe`, `fleet_audit`,
  `fleet_compliance`, and `fleet_schedule_*` — additive typed IPC, no secrets.
