# Phase 17 Rule Inventory

The executable source of truth is `crates/pc-intelligence/src/rules.rs` and `RULE_ENGINE_VERSION = phase17-rules-v1`.

| Rule | Domain | Condition | Result |
|---|---|---|---|
| P17-DRV-001 | Drivers | Present device explicitly lacks usable driver | DRIVER_MISSING |
| P17-DRV-002 | Drivers | Device has explicit PnP problem state without missing driver | DEVICE_PROBLEM |
| P17-DRV-003 | Drivers | Authoritative Driver Hub supplies update candidate | DRIVER_UPDATE_AVAILABLE |
| P17-WIN-001 | Windows | Integrity result is not a verified healthy result | WINDOWS_INTEGRITY_ATTENTION |
| P17-STO-001 | Storage | Explicit errors/critical warning or bounded warning evidence | STORAGE_RELIABILITY_CONCERN / STORAGE_ATTENTION |
| P17-MEM-001 | Memory | Current memory load >= 95% | HIGH_MEMORY_PRESSURE |
| P17-HW-001 | Hardware | Hardware/WHEA evidence exists | HARDWARE_ERROR_EVIDENCE |
| P17-CRASH-001 | Diagnostics | Crash evidence is not future-dated and is <=30 days old | RECENT_CRASH_EVIDENCE |
| P17-START-001 | Startup | >=3 high-impact startup entries | HIGH_STARTUP_FOOTPRINT |
| P17-CLEAN-001 | Cleanup | Reclaimable candidate >=512 MiB | CLEANUP_OPPORTUNITY |
| P17-UPD-001 | Updates | Verified AetherCore application update is available | APP_UPDATE_AVAILABLE |
| P17-CORR-001 | Drivers/Diagnostics | Driver change followed by crash within seven days, both current-window valid | POSSIBLE_DRIVER_REGRESSION |
| P17-CORR-003 | Hardware/Diagnostics | Recent hardware-error evidence and recent crash coexist | CRASH_WITH_HARDWARE_EVIDENCE |

A previously drafted correlation between Update Engine failure and Windows component-store corruption was removed during adversarial review because Update Engine represents AetherCore's own updater, not general Windows Update health. Retaining that rule would have created an unsupported servicing inference.
