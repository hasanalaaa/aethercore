# AetherCore Zenith Recursive Architecture Map

## Repository intelligence scan

At the transformed source tree: **34 Rust workspace members**, **82 Rust source files**, **25 Svelte components**, **34 TypeScript modules**, **9 UI CSS files**, and **69 PowerShell verification/release scripts**. A separate `aethercore-fuzz` Cargo package exists outside the primary workspace membership.

## Authority graph

```text
Svelte 5 renderer (untrusted presentation)
        |
        v
Tauri desktop host (non-elevated)
        |
        | typed Protobuf v7 / system-owned continuously held first-instance namespace / identification SQOS
        v
Maintenance Service (privileged service boundary)
        |
        +--> OperationKernel
        |      + cancellation registry
        |      + bounded principal EventBus/replay
        |      + ReadBudgetManager
        |      + MutationSupervisor
        |
        +--> OperationEngine
        |      + immutable action material + digest/coherence
        |      + plan state machine
        |      + one-shot consent consumption
        |
        +--> SQLite WAL durable safety/recovery ledger
        |
        +--> Worker-owned mutation lease
        |      + driver install
        |      + system repair
        |      + cleanup
        |      + startup changes
        |
        +--> Worker-owned expensive-read lease
        |      + driver discovery
        |      + repair assessment
        |      + cleanup discovery
        |      + startup discovery
        |      + diagnostics
        |
        +--> update orchestration / support export / idle intelligence

Explicit elevation boundaries:
Desktop -> consent intent ID -> elevated consent broker -> service approval
Desktop/service staged update -> update broker -> existing WiX/Burn authority
```

The renderer is never mutation authority. Before any production protocol frame is sent, the desktop verifies the opened fixed-name pipe is owned by the exact `NT SERVICE\AetherCoreMaintenance` service SID and that the registered maintenance service is running as a LocalSystem own-process service with `SERVICE_SID_TYPE_UNRESTRICTED`; service startup claims the first pipe instance, creates each successor listener before handing off the accepted session, and the client uses identification-only SQOS. The service then derives the connected principal from the named-pipe client token. The recursive pass further separates **authority** from **observation**: event/status watchers own neither a `MutationLease` nor a `ReadBudgetLease`.

## Local Rust dependency graph

Primary workspace local-package edges after transformation:

- `aethercore-desktop` -> `contracts`, `gpu-policy`, `ipc`, `support-bundle`, `update-download`, `update-engine`
- `aethercore-consent-broker` -> `contracts`, `ipc`
- `aethercore-maintenance-service` -> `cleaner`, `collector-runtime`, `contracts`, `diagnostic-engine`, `diagnostics`, `driver-hub`, `driver-install`, `idle-scheduler`, `ipc`, `operation-engine`, `operation-kernel`, `persistence`, `restore-point`, `security`, `startup-manager`, `support-bundle`, `system-repair`, `update-engine`
- `aethercore-ipc` -> `contracts`
- `aethercore-operation-engine` -> `persistence`
- `aethercore-operation-kernel` -> `contracts`, `operation-engine`, `persistence`
- `aethercore-security` -> `windows-foundation`
- `aethercore-windows-update` -> no local runtime package edge
- `aethercore-driver-hub` -> `collector-runtime`, `gpu-policy`, **`operation-kernel`**, `windows-pnp`, `windows-update`
- `aethercore-driver-install` -> `driver-backup`, `driver-hub`, `operation-engine`, `operation-kernel`, `persistence`, `restore-point`, `windows-pnp`, `windows-update`
- `aethercore-system-repair` -> `operation-engine`, `operation-kernel`, `persistence`, `windows-update`
- `aethercore-cleaner` -> `collector-runtime`, `operation-engine`, `operation-kernel`, `persistence`
- `aethercore-startup-manager` -> `collector-runtime`, `operation-engine`, `operation-kernel`, `persistence`
- `aethercore-hardware-telemetry` -> `collector-runtime`, `restore-point`
- `aethercore-crash-diagnostics` -> `collector-runtime`
- `aethercore-diagnostic-engine` -> `collector-runtime`, `crash-diagnostics`, `hardware-telemetry`, **`operation-kernel`**, `persistence`
- `aethercore-idle-scheduler` -> `collector-runtime`, `contracts`, `operation-kernel`, `security`, `windows-foundation`
- `aethercore-update-engine` -> `operation-kernel`, `persistence`
- `aethercore-update-broker` -> `contracts`, `ipc`, `update-engine`
- `aethercore-update-manifest-tool` -> `update-engine`
- `aethercore-support-bundle-verify` -> `support-bundle`
- `aethercore-ga-probe` -> `contracts`, `ipc`
- leaf/no-local-edge packages include `contracts`, `windows-foundation`, `collector-runtime`, `diagnostics`, `persistence`, `gpu-policy`, `windows-pnp`, `restore-point`, `support-bundle`, `update-download`, `install-hardener`

A local dependency-cycle scan over all Cargo packages found **0 local package cycles** after adding the worker-budget dependencies to `driver-hub` and `diagnostic-engine`.

## Critical data paths

### Mutation path

```text
native evidence
 -> typed immutable PlanAction
 -> immutable_json + digest
 -> duplicate-column coherence gate
 -> verified PlanView
 -> principal-bound one-shot consent
 -> consume consent + Preflight
 -> MutationSupervisor exact lease
 -> lease moved into privileged worker
 -> Protected / Executing / Verifying
 -> durable terminal or reboot/recovery state
```

The watcher is parallel observation only:

```text
worker/coordinator state -> watcher -> EventBus -> IPC -> renderer
```

Watcher loss cannot release machine mutation authority.

### Expensive read path

```text
request -> ReadBudgetManager exact workload lease
        -> lease moved into collector worker
        -> typed snapshot
        -> watcher/EventBus/IPC
```

The budget remains occupied for actual collector lifetime, not for polling lifetime.

### Streaming path

`typed coordinator state -> principal EventBus -> bounded replay -> bounded subscriber queue -> bounded IPC outbound queue -> persistent desktop session`. Lag or lost replay produces explicit `StreamReset` + hydration. Inactive principal replay state is bounded to 128 retained streams under normal conditions.

### Request admission path

`global sessions <= 32 -> per-SID sessions <= 4 -> per-session requests <= 8 -> global accepted request workers <= 64 -> deadline/cancellation -> handler`.

This makes reconnect churn incapable of bypassing the process worker budget.

## Risk matrix

| Risk | Impact | Treatment after pass |
|---|---:|---|
| Consent summary differs from frozen actions | High | Fixed fail-closed at material decode/view boundary |
| Watcher releases mutation authority before worker completes | High | Fixed: mutation worker owns exact lease |
| Read watcher releases expensive-read budget before collector completes | High | Fixed: collector owns exact budget lease |
| Reconnect churn accumulates detached native workers | High | Fixed: process-global worker budget |
| Thread creation fails after consent/state acquisition | High | Fixed: fallible named spawn + path-specific rollback/failure |
| Historical EventBus/update owner state grows indefinitely | Medium | Fixed with quiescent-owner retention; live/critical state protected |
| Token-only source audit reports false PASS on malformed Rust | Medium governance | Fixed with malformed-declaration meta-regression; compiler still authoritative |
| Fixed-name pipe namespace squatting / extra server instance | High | Mitigated in source: exact service-SID owner/server authority + LocalSystem own-process service with `SERVICE_SID_TYPE_UNRESTRICTED` + first-instance startup + successor-before-handoff listener continuity + exact AU mask excluding create-instance + identification SQOS; live Windows owner/DACL/token probes remain mandatory |
| Non-reading peer stalls synchronous named-pipe I/O | Medium | Mitigated: bounded queues + cross-thread `CancelSynchronousIo` + post-shutdown write fence; native cancellation timing still requires proof |
| Dependency drift / unreproducible release | Critical release | Existing lockfile freeze remains fail-closed prerequisite |
| Windows provider/device/accessibility variance | Medium/High | Native OEM/device/AT/DPI matrix remains mandatory |

## Improvement roadmap

1. **Trusted dependency freeze:** generate and review `Cargo.lock` and `pnpm-lock.yaml`; retain all `--locked` release gates.
2. **Qualified Windows compiler pass:** `cargo fmt`, workspace `clippy -D warnings`, targeted recursive tests, complete workspace tests and UI production build.
3. **Authority fault injection:** force worker/watcher/thread creation failures and verify mutation/read leases follow worker lifetime exactly.
4. **Transport qualification:** stress the successor-listener namespace continuity under rapid disconnect/squatter races, then stress the cross-thread synchronous-I/O cancellation path with slow/non-reading peers, service stop, disconnect, cancellation races and handle/thread telemetry. Move to overlapped I/O only if this evidence shows teardown is not deterministic enough.
5. **Fleet/device matrix:** real WUA/PnP/WMI/storage/SMART/OEM/driver servicing, power transitions and extended soak.
6. **Physical UX qualification:** Narrator, keyboard, Arabic RTL, high contrast, reduced motion/transparency, 100–250% DPI, 60/120/144 Hz and pointer cancel/lost capture.
7. **Fleet readiness only after local trust is proven:** any remote management plane must introduce explicit device identity, tenant isolation, signed policy and a separate audited authority model rather than extending renderer authority.
