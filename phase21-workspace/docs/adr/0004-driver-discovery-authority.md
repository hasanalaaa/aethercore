# ADR-0004: Windows owns driver applicability; AetherCore owns deterministic matching

**Status:** Accepted

## Context

A driver-management product needs to distinguish three different concepts: what hardware is present, what driver is currently bound, and what update Windows considers applicable. Treating a larger version number or a web catalog entry as proof of applicability can select the wrong package for an OEM-specific device.

## Decision

- Use SetupAPI and Configuration Manager for the current PnP inventory.
- Use Windows Update Agent for live Windows-managed driver applicability.
- Expand `IWindowsDriverUpdate4::WindowsDriverUpdateEntries` when available.
- Pair WUA entries to the current inventory only through exact normalized Hardware IDs, then exact Compatible IDs.
- Never infer applicability from version number, model-name similarity, manufacturer name, or a third-party catalog.
- Keep unmatched WUA entries observable but unattached.
- Treat NVIDIA/AMD/Intel display adapters as vendor-managed and non-selectable by default.
- Keep firmware-class offers visible but non-selectable until a separately reviewed firmware workflow exists.

## Consequences

This architecture may show fewer “updates” than aggressive third-party driver tools, but every attached candidate has explicit Windows applicability evidence and a concrete current PnP target. It also means the UI cannot truthfully promise “newest driver worldwide”; it can say that Windows currently offers an applicable driver update.
