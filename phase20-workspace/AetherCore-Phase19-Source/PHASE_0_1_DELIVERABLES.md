# Phase 0 + Phase 1 Deliverables

## Completed foundation

- Cargo 2024 workspace and frontend workspace.
- Product contract, threat model, and architecture decision records.
- Versioned Protocol Buffer contracts with vendored `protoc`.
- Bounded local named-pipe transport with remote-client rejection, explicit DACL, and bounded in-flight concurrency.
- Native caller PID/elevation inspection plus exact broker image-path validation for sensitive IPC.
- SQLite WAL persistence, schema migration, event journal, challenges, and grants.
- Structured local JSONL diagnostics with bounded log rotation.
- Durable plan state machine and simulation-only Phase 1 plan, with restart and uncommitted-transaction fault-injection tests.
- Windows Service host + console development host.
- UAC Consent Broker and exact-digest challenge/grant flow.
- Tauri command bridge to the native service.
- Initial luxury Svelte dark-mode shell with live service status and plan timeline.
- Windows bootstrap, one-command setup/run, development runner, service install/uninstall, and verification scripts.
- Windows CI definition.

## Intentionally deferred

- Device/driver inventory and Windows Update Agent integration (Phase 2).
- Restore points and driver mutation (Phase 3).
- DISM/SFC and cleanup providers (Phase 4).
- Startup/service optimization (Phase 5).
- Hardware and crash diagnostics (Phase 6).
- Production WiX packaging, signing, updater, fuzzing, and external security review (Phase 8).
