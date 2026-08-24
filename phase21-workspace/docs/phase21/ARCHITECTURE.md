# Phase 21 — Timeline Intelligence & Recurrence Reasoning

Status: source-complete on this machine; Windows-native qualification remains open
(see QUALIFICATION_DEBT.json). Contract: PCD-TIMELINE-INTELLIGENCE in
PRODUCT_CAPABILITY_DEBT.json — "Future timeline consumes issue/repair/reboot/
verification/recurrence/escalation events without rewriting their historical truth."

## 1. Domain crate — crates/timeline-intelligence

### Typed event model (src/model.rs)

`TimelineEvent` carries exactly what persisted history already proved:

| Field | Source |
|---|---|
| `source_id` | persistence row identity (`journal:<seq>`, `execution:<plan_id>`, `repair-event:<event_id>`, `scan:<scan_id>`) |
| `class` | `Operation` / `Finding` / `Verification` / `Recovery` / `Escalation` (PCD event families) |
| `domain`, `code` | verbatim from the persisted rows, never rewritten |
| `outcome` | `Succeeded` / `Failed` / `Neutral` mapped by a conservative vocabulary |
| `observed_unix_ms` | the row's own timestamp (completion time when present), never wall-clock at mapping |
| `semantic_identity_sha256` | sha256 over `(class ordinal, len(domain), domain, len(code), code)` |

The semantic identity deliberately excludes timestamps, identifiers and free-text
detail — the same discipline as the Phase 17.1 machine-state fingerprint. Length
prefixes make ("ab","c") and ("a","bc") uncollidable.

### Deterministic construction (src/engine.rs)

- Ingestion deduplicates on `(semantic identity, observation timestamp)`: replayed or
  re-reported rows describing the same fact collapse; distinct facts never do.
- Retention is hard-bounded (`MAX_TIMELINE_EVENTS = 2000`); exceeding it is a typed
  error, not silent truncation.
- Ordering is total: `(observed_unix_ms, semantic_identity_sha256, source_id)` — so
  identical input yields byte-identical output regardless of insertion order.
  The digest covers sorted events plus emitted pattern content.
- Events stamped after the freshness watermark are rejected with
  `TimelineError::FutureTimestamp`; capacity overflow is `TimelineError::CapacityExceeded`.

### Recurrence reasoning

Only failure-shaped outcomes feed detection. A pattern is emitted only when its full
evidence matrix is satisfiable:

1. ≥ `MIN_OCCURRENCES_FOR_PATTERN` (3) distinct occurrences;
2. strictly increasing observation times (causal-ordering evidence);
3. every consecutive gap inside `[0, MAX_RECURRENCE_GAP_MS]` (30 days) — negative or
   over-window distances disqualify the entire run, so hostile clocks never become evidence;
4. bounded citations (`MAX_PATTERN_EVIDENCE = 32`, first and last always present).

Confidence classes: **Strong** (≥5 occurrences, all gaps within ±25 % of mean),
**Moderate** (≥4, ±50 %), **Weak** (floor run, irregular spacing). A group failing any
clause emits nothing. Patterns cap at `MAX_PATTERNS = 64`, kept in stable identity order.

Correlation over time is all this engine claims. It never asserts causation.

### Read-only ingestion (src/ingest.rs)

Consumes four existing tables through public accessors:
`plan_events` (via journal reader), `repair_timeline_events`,
`maintenance_executions`, `intelligence_scans`. Two additive read-only accessors were
added to `aethercore-persistence` (`repair_timeline_events_for_owner`,
`maintenance_executions_for_owner`). No INSERT/UPDATE/DELETE/DDL exists in the crate;
no migration was added or modified; foreign principals cannot leak (every query is
owner-scoped). Future-stamped rows are quarantined during ingestion.

## 2. Wire contract — additive only

New file `crates/contracts/proto/timeline.proto`. Tags consumed:

| Surface | Tag | Name |
|---|---|---|
| EventKind | 25 | `EVENT_KIND_TIMELINE_PAGE` |
| EventEnvelope payload | 34 | `timeline_page` (stream normalization) |
| Request payload | 78 | `get_timeline_page` |
| Request payload | 79 | `get_recurrence_patterns` |
| Response payload | 45 | `recurrence_patterns` |
| Response payload | 46 | `timeline_page` |

No existing tag was renumbered; frozen ranges 0–21, 22–24, payloads 31–33 and response
tag 44 are asserted unchanged by the extended audit's wire-freeze gate.

## 3. Service integration — services/maintenance-service/src/timeline.rs

`TimelineCoordinator` builds timelines on demand from persisted history. Handlers are
principal-scoped through the router exactly like every other domain: the principal key
comes from `peer.binding_key()` after the kernel checkpoint and session-binding check,
never from request content. Page size is clamped server-side to `[1, MAX_PAGE_SIZE]`
(200); `before_sequence` is an opaque cursor into the ordered timeline. The page is
republished through the ordered Event Bus as `EVENT_KIND_TIMELINE_PAGE`, normalized by
the desktop host like every other stream event.

## 4. Desktop + renderer

Two Tauri commands (`get_timeline_page`, `get_recurrence_patterns`) follow the existing
request/retry/extract pattern. The renderer gains typed contracts
(`apps/ui/src/lib/contracts.ts`), a `timelinePage` stream-state slice, and a controller
(`apps/ui/src/features/timeline/controller.ts`) that fetches on demand.

`TimelinePanel` mounts inside the Activity route — the durable-journal home — because
the timeline is the journal viewed as history, not a new navigation destination; the
keyboard-shortcut contract therefore stays untouched. The component obeys the Apple
interaction contract: press feedback via `fluidPress` on pointerdown, spring-driven
motion through shared primitives, reduced-motion/reduced-transparency via the shared
preference store, and TechnicalText isolation for timestamps/digests/codes under bidi.
Arabic plurals use the six-form catalog (`unit.occurrence`).

## 5. i18n

23 `timeline.*` keys added to both catalogs; programmatic EN/AR parity for them is part
of the extended audit (Gate P21-7), alongside the pre-existing perf-key parity gate.

## 6. Verification summary

Run on macOS (development host): entry gates reproduced (verify PASS/620, audit 43/PASS,
workspace tests 327 passed → now 343+ with the new suites), extended audit 159 checks
PASS, svelte-check 0 errors / 17 warnings (unchanged), binary-safe patch round-trip
0 missing / 0 extra / 0 mismatch ×2 rebuilds, master zip deterministic ×2.

NOT_EXECUTED here: Windows-native service behavior, installer, update-broker flows —
unchanged scope-wise from Phase 20 but still owed native qualification.
