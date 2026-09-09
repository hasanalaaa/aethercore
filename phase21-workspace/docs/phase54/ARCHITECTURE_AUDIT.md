# Phase 54 — application-wide architecture audit

## Scope

The live source tree was reviewed across the desktop bridge, Svelte state and
feature controllers, maintenance-service IPC router, operation kernel and
supervisor, persistence, telemetry, repair/cleanup/startup/driver domains,
local intelligence, update authority, support bundles, WiX packaging, and CI.
Historical `PHASE_*_BINARY_SAFE_PATCH` copies were excluded from architectural
decisions. The external Graphify index was unavailable, so live source and the
existing phase ledgers remain authoritative.

## Runtime map

`apps/desktop` is the Tauri boundary. It forwards typed protobuf requests to the
maintenance service and exposes the UI transport. `apps/ui` keeps one kernel
session, reducers, and feature stores; controllers issue typed read or mutation
requests. The Windows service authenticates the named-pipe client, binds a
principal key, enforces quotas/deadlines/cancellation, and routes requests.

The service composition wires SQLite persistence, the operation engine and its
mutation supervisor, recovery/event telemetry, domain coordinators, performance
sampling, care orchestration, and the ephemeral intelligence coordinator.
Mutation paths follow scan → plan → consent/UAC → start → event/snapshot
verification. Update paths add signed identity/manifest checks, bounded HTTPS
download, Authenticode verification, immutable apply plans, and rollback state.

## Findings addressed in this cycle

* Ephemeral insight state was process-global. `list_insights` and
  `dismiss_insight` therefore crossed authenticated principals. The registry is
  now keyed by the authenticated principal and all list/dismiss/replace paths
  use that key.
* Overview returned as soon as an existing window was found, so its newest UI
  value could remain old. It now refreshes the current snapshot after applying
  the bounded window.
* A transient Overview read stopped future polling permanently. The loop now
  schedules the next bounded read after failures, allowing service reconnects to
  recover without navigation.
* Local dismissal now applies the server's returned session response, so the
  card disappears immediately and consistently with the service.
* Insight evidence uses each insight's engine, and security finding citations
  have an explicit UI surface and translations.
* Evidence detail truncation is UTF-8 boundary safe; Arabic input no longer can
  panic the intelligence core.

## Remaining release gates

The source-level checks do not prove a Windows installer release. The protected
Windows pipeline still requires approved dependency-freeze evidence, a clean
Rust/UI build, WiX MSI validation, WebView2 and VC runtime provenance, PE
hardening, Authenticode signing, and disposable-VM install/upgrade/uninstall
qualification. The current ARM64 VM contains historical MSI artifacts, but an
artifact is not evidence that the current source revision was rebuilt and
qualified. Public promotion must therefore use the protected release workflow
after those gates pass.

The repository currently stores workflows under `phase21-workspace/.github`.
GitHub only executes workflows under the repository root `.github`; the release
process must either make `phase21-workspace` the repository root or add a
reviewed root workflow that invokes the nested scripts with explicit working
directories.
