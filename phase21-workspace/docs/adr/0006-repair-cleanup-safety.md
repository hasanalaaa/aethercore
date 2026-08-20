# ADR 0006 — Repair and cleanup remain typed, allowlisted maintenance transactions

**Status:** Accepted
**Date:** 2026-08-18

## Context

Phase 4 introduces Windows component repair and filesystem deletion. Both capabilities are dangerous if exposed as generic command execution, caller-supplied paths, registry heuristics, or broad recursive deletion.

## Decision

System Repair exposes only a fixed DISM/SFC/CHKDSK workflow. Cleanup exposes only service-owned providers that mint exact candidates. Both become immutable operation-engine plans and require the existing digest-bound UAC authorization before mutation.

Cleanup plans persist exact file evidence from scan time. At deletion time each file is reopened, checked for reparse traversal, resolved to a final handle path, compared with frozen evidence, and deleted by the validated handle. Unknown or changed targets are skipped.

Driver installation, Repair, and Cleanup share the same machine-wide AetherCore mutation lock. Interrupted Repair/Cleanup mutations are never automatically replayed.

## Consequences

- The product cannot advertise arbitrary “one-click registry repair,” universal junk discovery, or automatic removal of unknown files.
- Some reclaimable space will intentionally remain because locked, changed, redirected, or unsupported targets are skipped.
- Per-profile cleanup requires explicit review because a LocalSystem service must not infer that a profile belongs to the current interactive user.
- Offline CHKDSK repair and reboot scheduling remain future explicit workflows rather than being smuggled into Phase 4.
