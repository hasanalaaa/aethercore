# ADR 0007 — Passive-default, evidence-bound startup management

## Status

Accepted for Phase 5.

## Context

Startup optimization is unusually easy to over-automate. A list of autostart entries does not prove that disabling an item is safe or that it materially slows boot. Registry values, task definitions, files, and service configurations can also change between scan, consent, and execution.

## Decision

AetherCore uses an explicit three-state recommendation decision: `Unreviewed`, `KeepEnabled`, and `Disable`. `Unreviewed` is always the initial state and never implies consent. `KeepEnabled` and `Unreviewed` create no mutation action.

Only `Disable` decisions for current service-issued, manageable, unprotected items may become immutable plan actions. Each action freezes exact original state and typed intended state. The existing digest-bound UAC broker authorizes only that exact plan.

Service changes are further constrained: Phase 5 never stops a running service and never sets `SERVICE_DISABLED`; it changes eligible automatic services to Demand/Manual for future starts. Service disable and restore each require the additional service confirmation.

Every change is journaled before mutation. Restore is a new plan/change record linked to the original rather than an in-place undo that destroys history. Restart recovery observes native state and never replays an interrupted mutation automatically.

Boot impact remains `Unknown`/`InsufficientEvidence` unless direct trustworthy evidence can be bound to the item. AetherCore does not fabricate impact categories from the mere fact that an item autostarts.

## Consequences

- User inaction has a mechanically testable zero-mutation guarantee.
- UI compromise cannot turn a protected item into an authorized native target because policy is repeated in the service layer.
- Reversibility is auditable rather than best-effort.
- Startup optimization is deliberately less aggressive than many PC utilities.
- Some potentially removable items remain observation-only until stronger safety/evidence adapters exist.
