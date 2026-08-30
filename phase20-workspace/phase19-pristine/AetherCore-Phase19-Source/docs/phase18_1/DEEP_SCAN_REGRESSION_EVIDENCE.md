# Deep Scan Driver Truth Regression Evidence

PC Intelligence now distinguishes:

- concrete `DRIVER_UPDATE_AVAILABLE`
- `DRIVER_MANAGEMENT_AUTHORITY_AVAILABLE`
- `DRIVER_UPDATE_STATUS_UNKNOWN`
- `DRIVER_AUTHORITY_COVERAGE_INCOMPLETE`
- missing driver / device problem

Management guidance is informational and no longer creates a driver-update finding. A driver update is not treated as a system-health failure by itself. Firmware remains protected and missing/problem states remain separate from maintenance guidance.

Legacy `VENDOR_UTILITY_REQUIRED` identifiers remain accepted only where needed for persisted finding lifecycle/remediation compatibility; Phase 18.1 generation paths do not emit a management utility as update evidence. User-facing legacy wording was rewritten to state that utility presence does not prove an update.
