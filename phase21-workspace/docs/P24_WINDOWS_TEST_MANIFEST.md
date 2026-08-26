# P24 Windows Test Manifest — Ready-Made Qualification Checklist

These tests are `#![cfg(windows)]` — they compile and run ONLY on a Windows host. They
are the exact, ready-made qualification checklist for the Windows lane (QD-028-001 /
P24). Run on the Windows host with:

```powershell
cargo test --workspace --jobs 2
```

| # | Test | Crate / File | What it proves |
|---|------|--------------|----------------|
| 1 | `live_cleanup_scan_returns_only_service_minted_categories` | `aethercore-cleaner` · crates/cleaner/tests/live_scan.rs | Live cleanup scan returns ONLY service-minted categories (no invented cleanup targets) on a real Windows filesystem. |
| 2 | `exports_a_bound_oem_driver_package_with_manifest` | `aethercore-driver-backup` · crates/driver-backup/tests/live_backup.rs | Bound OEM driver package exports with a valid manifest through the real DriverStore. |
| 3 | `creates_verifies_and_cancels_a_fresh_restore_point` | `aethercore-restore-point` · crates/restore-point/tests/live_restore.rs | Full VSS lifecycle: create → verify → cancel a fresh system restore point. |
| 4 | `live_windows_repair_assessment_is_read_only_and_completes` | `aethercore-system-repair` · crates/system-repair/tests/live_assessment.rs | DISM/SFC repair assessment completes READ-ONLY against the live component store. |
| 5 | `inventories_present_hardware_nodes` | `aethercore-windows-pnp` · crates/windows-pnp/tests/live_inventory.rs | PnP inventory enumerates present hardware nodes via the real configuration manager. |
| 6 | `discovers_live_driver_offers_without_installing` | `aethercore-windows-update` · crates/windows-update/tests/live_wua.rs | WU driver-offer discovery works without installing anything. |

## Additional cfg(windows)-gated suites (compile-only proof on unix)

The following production modules are `#[cfg(windows)]` and are byte-frozen; their
correctness is enforced by compilation in CI-matrix plus the sealed-P26+ patch chain:
`crates/windows-foundation`, `windows-pnp`, `windows-update`, `driver-acquisition`
(WU flow), `system-repair` (DISM/SFC adapters), consent-broker/update-broker apps,
named-pipe IPC transport, service SCM host.
