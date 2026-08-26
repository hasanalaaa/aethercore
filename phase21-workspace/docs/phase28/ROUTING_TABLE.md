# ROUTING_TABLE.md — aetherctl ↔ wire contract (Phase 28)

One truth surface (CX-3): every service-backed subcommand maps onto an EXISTING v7 wire
tag. No tags were invented or renumbered this phase; scripts/phase28-adversarial-audit.py
extracts request/response fields from `crates/contracts/proto/operations.proto` and fails
the build if any row below drifts from reality (gate p28-routing-real-*-tag).

Request tag = field number inside `message Request`; Response tag = field number inside
`message Response`. Exit codes are defined in EXIT_CODES.md.

## Service-backed lanes

| Subcommand | Request tag / Response tag | Exit codes (ok / error) | Offline availability |
|---|---|---|---|
| `doctor` | 40 (get_diagnostics_snapshot) / 25 (diagnostics_snapshot) | 0 · 3 4 5 | none |
| `perf start [--interval-ms]` | 71 (start_perf_sampling) / — (empty payload) | 0 · 3 4 5 | none |
| `perf stop` | 72 (stop_perf_sampling) / — | 0 · 3 4 5 | none |
| `perf snapshot` | 73 (get_performance_snapshot) / 41 (performance_snapshot) | 0 · 3 4 5 | none |
| `perf report` | 74 (get_bottleneck_report) / 42 (bottleneck_report) | 0 · 3 4 5 (insufficient evidence is a typed 5) | none |
| `optimize plan [--findings]` | 75 (create_optimization_plan) / 44 (optimization_plan) | 0 · 3 4 5 | none |
| `optimize start [--plan-id]` | 76 (start_optimization) / 43 | 0 · 3 4 5 — the service currently refuses execution (`perf.error.executionRequiresConsent`) and the CLI surfaces that typed answer verbatim | none |
| `optimize status [--plan-id]` | 77 (get_optimization_status) / 43 | 0 · 3 4 5 | none |
| `timeline page [--size --before]` | 78 (get_timeline_page) / 46 (timeline_page) | 0 · 3 4 5 | none |
| `timeline patterns` | 79 (get_recurrence_patterns) / 45 (recurrence_patterns) | 0 · 3 4 5 | none |
| `care status` | 82 (get_care_status) / 47 (care_status) | 0 · 3 4 5 | none |
| `care start [--non-interactive]` | 80 (start_care_run; flow precedes it with 82 status fetch and — only after digest confirmation — 81 grant, same connection) / 47 | 0 · 3 4 6 (consent discipline below) | none |
| `care cancel` | 83 (cancel_care_run) / 47 | 0 · 3 4 5 | none |
| `care consent-grant` | 81 (grant_care_session_consent) / — | 0 · 3 4 5 | none |
| `insights list` | 84 (list_insights) / 48 (insights_response) | 0 · 3 4 5 | none |
| `insights explain [--question]` | 85 (request_insight) / 48 | 0 · 3 4 5 | none |
| `insights dismiss --insight-id` | 86 (dismiss_insight) / 48 | 0 · 3 4 5 | none |
| `scan start` | 65 (start_deep_scan) / 38 (deep_scan_snapshot) | 0 · 3 4 5 | none |
| `scan cancel --scan-id` | 66 (cancel_deep_scan) / 38 | 0 · 3 4 5 | none |
| `scan status` | 67 (get_deep_scan_snapshot) / 38 | 0 · 3 4 5 | none |
| `scan history [--limit]` | 68 (get_deep_scan_history) / 39 (deep_scan_history) | 0 · 3 4 5 | none |

## Embedded (offline) commands — strictly read-only, no daemon required

| Subcommand | Wire usage | Exit codes (ok / error) | Offline availability |
|---|---|---|---|
| `about` / `version` | none | 0 · 7 8 | full |
| `capabilities` | static matrix from crates/platform-capabilities (same data the service serves under tag 87) | 0 · 7 8 | full |
| `engine-source` | none (compile-time selection parity asserted against the service's engine_source()) | 0 · 7 8 | full |
| `telemetry-once [--interval-ms]` | none — one REAL snapshot from the cfg-selected PerfPlatform provider | 0 · 7 8 | full |
| `self-check [--load-model]` | none — manifest schema validation + streaming sha256, fail-closed; model loading ONLY behind `--load-model` (opt-in cargo feature, otherwise typed exit 7) | 0 · 6 7 8 | full |
| `service detect` | read-only socket probe + PID-file liveness cross-check | 0 · 7 8 | full (that is its purpose) |
| `service units --print` | none — prints packaging artifacts to stdout, writes NOTHING | 0 · 8 | full |
| `help` | none | 0 · | full |

## Consent discipline (`care start`)

1. Fetch current care status (tag 82); print the composed plan digest.
2. Consent requires an interactive text-mode confirmation: the operator retypes the first
   16 hex chars of the printed digest. Default answer is NO. A wrong digest is refused
   (typed exit 6) BEFORE any consent RPC leaves the process.
3. Only then does THIS principal connection execute grant (81) followed immediately by
   start (80) over the same connection, so the service-side per-principal session registry
   sees grant+start from one principal.
4. `--non-interactive` and `--output json` NEVER auto-consent: typed exit 6,
   message_key `cli.consent.interactiveConfirmationRequired`.
5. Empty plan digest (no eligible plan composed) → typed exit 6, `cli.care.noPlanDigest`.

## Detection states

| State | Meaning |
|---|---|
| `Reachable { pid }` | endpoint answers; pid present only when a PID file records a LIVE process (kill(pid,0) cross-check) |
| `Offline` | no endpoint file and no live daemon on record |
| `StaleEndpointRecovered` | endpoint file exists but nothing lives behind it (SIGKILL/crash leftover); the DAEMON rebinds it on next start — proven live in tests/phase28_cli_matrix.rs; the CLI never deletes anything |

Offline + service-backed command ⇒ typed envelope, exit 3, always within `--timeout-ms`.

## Deferred RPC lanes (honest scope statement)

These v7 lanes exist but are deliberately NOT exposed in Phase 28, each with rationale:

| Lane | Tags | Rationale |
|---|---|---|
| updates 46–52, 58–64 (check/stage/install/upload) | 46–64 | update flows require the desktop trust UX and signed-manifest choreography; a headless lane would bypass review gates — deferred to a future phase with explicit governance design |
| driver-policy (seal remediation plan / candidate policy) | 69–70 | Windows-only capability (honest matrix says notAvailable elsewhere); CLI surface waits for the P24 Windows lane workstream (QD-028-001) |
| support-bundle preview/prepare/chunk/export | 53–57 | chunked multi-call protocol designed for GUI export flows; headless export needs its own security review of where bytes land on disk |
| generic consent intents | 42–44 | plan-level consent intents belong to the desktop consent broker UX; session-scoped CARE consent IS covered above because it is connection-bound and non-persistent |

Wire freeze continues: requests ≤88, responses ≤50, EventKind ≤28, EventEnvelope ≤37 —
asserted by gate p28-wirefreeze-*.
