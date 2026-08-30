# Phase 17.1 Qualification Debt Status

Phase 17.1 does not introduce a new native product capability. It corrects semantic ownership, persistence, fingerprinting, and rule behavior within the existing Phase 17 Deep Scan architecture.

Accordingly the ledger retains the existing 12 capability IDs instead of duplicating debt.

Updated scenarios in the existing ledger explicitly include:

- `P17-QD-001`: rapid restart, stale cancellation and stale cleanup across scan generations;
- `P17-QD-004`: strong/moderate/weak WHEA/crash separations and causal-order checks;
- `P17-QD-007`: old native worker exit after a newer generation begins;
- `P17-QD-011`: deployed schema-v10 → v11 database continuity.

No entry is marked qualified merely from source inspection. Windows service runtime, native provider behavior, WebView/Tauri accessibility, deployed SQLite WAL upgrade, and representative hardware performance remain deferred.
