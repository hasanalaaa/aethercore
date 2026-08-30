# Phase 2 — Smart Driver Hub Deliverables

## Exit objective

Phase 2 turns the Phase 0/1 platform into a live, read-only Windows driver intelligence surface without crossing into driver installation. The maintenance service owns the hardware/update snapshot; the UI only renders it and stages eligible candidate IDs for a future Phase 3 plan.

## 1. Native PnP inventory

Implemented in `crates/windows-pnp`.

- Enumerates present PnP nodes through `SetupDiGetClassDevsW(DIGCF_PRESENT | DIGCF_ALLCLASSES)` and `SetupDiEnumDeviceInfo`.
- Reads device instance IDs, friendly name/description, class/class GUID, manufacturer, enumerator, location, Hardware IDs, and Compatible IDs.
- Uses `CM_Get_DevNode_Status` for status flags and Device Manager problem code.
- Treats Device Instance ID and devnode-status reads as consistency-critical; failure aborts the inventory snapshot instead of substituting empty identity or healthy/default status.
- Accepts the problem number only when `DN_HAS_PROBLEM` is set, preventing stale/non-applicable problem values from becoming false positives.
- Classifies `DN_HAS_PROBLEM` + problem code 28 as `missing_driver`.
- Reads currently bound driver provider, version, INF, and date from the device driver registry key returned by SetupAPI.
- Excludes the Windows `SWD` software-device enumerator from the hardware-focused Phase 2 snapshot.
- Fails the scan on an unexpected SetupAPI enumeration error instead of silently treating it as end-of-list.

Unit coverage includes MULTI_SZ parsing, ID normalization, and Code 28 semantics. An ignored Windows-only physical smoke test is in `crates/windows-pnp/tests/live_inventory.rs`.

## 2. Live Windows Update discovery

Implemented in `crates/windows-update`.

- Initializes COM in MTA mode and creates the official WUA `UpdateSession`.
- Sets a product-specific `ClientApplicationID`.
- explicitly sets the searcher online while retaining WUA's configured update source/policy.
- Searches `IsInstalled=0 and Type='Driver' and IsHidden=0`.
- Reads update identity/revision, title, size range, support metadata, provider/manufacturer/model/class, driver date, hardware ID, problem number, and device status.
- Uses `IWindowsDriverUpdate4::WindowsDriverUpdateEntries` when present so one WUA update can emit the device-specific entries for every applicable target; falls back to the base `IWindowsDriverUpdate` entry on older agents.
- Checks `ISearchResult::ResultCode`: `SucceededWithErrors` is retained with an incomplete-results warning; failed/aborted search results are rejected as discovery failures.
- Converts WUA `DECIMAL` download sizes to `u64` without floating-point size loss.
- Never calls `IUpdateDownloader`, `IUpdateInstaller`, or any installation API.
- Never claims a target driver version unless a conservative dotted version can be extracted from the Windows title.

Unit coverage includes conservative target-version parsing and OLE Automation driver-date conversion. An ignored Windows-only live scan test is in `crates/windows-update/tests/live_wua.rs`.

## 3. Matching and scan coordinator

Implemented in `crates/driver-hub`.

Read-only scan states:

```text
Idle
  -> InventoryScanning
  -> UpdateSearching
  -> Matching
  -> Ready
  |-> Failed
```

Every scan receives a new `scan_id` and monotonically increasing `inventory_epoch`. Results are deliberately volatile because driver applicability is time-sensitive; future mutation plans must refer to service-issued candidate IDs plus the inventory epoch and revalidate before changing the system.

Matching rules:

1. Normalize PnP IDs case-insensitively.
2. Match a WUA entry to exact current Hardware IDs first.
3. Only if no Hardware ID matches, try exact Compatible IDs.
4. Never attach an unmatched offer based on model/vendor/version heuristics.
5. Preserve unmatched WUA entries for diagnostics rather than silently discarding or guessing.
6. Generate deterministic candidate IDs from update ID + revision + device instance ID.
7. Deduplicate candidates while preserving WUA identity/revision; when the same update matches both exact Hardware ID and Compatible ID for one device, exact Hardware ID evidence wins deterministically.

Tests cover exact matching, Compatible ID fallback, no-guess behavior, GPU exclusion, firmware manual-review exclusion, overlapping-scan rejection, inventory failure, WUA-degraded completion, and asynchronous scan coordination with fake backends.

## 4. GPU and firmware safety policy

Implemented in `crates/gpu-policy`, `crates/driver-hub`, and `apps/desktop`.

- Detects display-class devices by class name/GUID.
- Identifies NVIDIA (`VEN_10DE`), AMD (`VEN_1002`), and Intel (`VEN_8086`), with manufacturer fallback.
- Detects known machine companion applications from trusted Program Files locations.
- Marks **every display-class offer** `vendor_managed=true`, `selectable=false`, and `selected_by_default=false`, including unknown/OEM display vendors.
- NVIDIA/AMD/Intel adapters receive the richer recognized vendor metadata/action; unknown display vendors remain manual-review only.
- Exposes app name/installed state to the UI.
- The shared GPU policy owns fixed Program Files companion-app paths plus HTTPS vendor fallbacks. The Tauri command receives only the vendor string, launches the known local app when present, otherwise opens the fixed official fallback, and never executes `support_url` supplied by WUA metadata.
- Firmware-class devices/offers are marked `firmware_managed=true`, `selectable=false`, and `selected_by_default=false`; they remain visible as manual-review evidence but cannot enter the Phase 2 selection set.

## 5. IPC and service integration

Updated `crates/contracts`, `crates/ipc`, and `services/maintenance-service`.

New typed requests:

- `StartDriverScanRequest`
- `GetDriverHubSnapshotRequest`

New response model includes scan state, summary, full device list, installed-driver metadata, candidate metadata, GPU policy, unmatched WUA entries, and warnings.

IPC framing is split by direction:

- request maximum: 256 KiB;
- response maximum: 8 MiB.

This keeps the attack-facing request surface small while allowing large but bounded PnP snapshots on hardware-heavy systems.

## 6. Desktop and Svelte dashboard

The Drivers screen now provides:

- live scan state without fake percentages;
- PnP/WUA/matching stage indicator;
- present-device, selectable-offer, missing-driver, protected-display, and recognized-vendor GPU metrics;
- device search and All/Updates/Problems/Missing/Display filters;
- current driver provider/version/date/INF metadata;
- Windows offer title, target version only when conservatively parsable from the WUA title (with source disclosure), driver date, and download range;
- Device Manager problem states and Code 28 missing-driver state;
- per-candidate selection only for eligible non-GPU, non-firmware offers;
- review-only selected download aggregate;
- expandable Hardware IDs and Compatible IDs;
- vendor-managed GPU explanation plus a safe companion-app-or-official-support action; firmware offers remain visible but locked for manual review.

No Phase 2 UI action can download or install a driver.

## 7. Validation entry points

Normal deterministic gate:

```powershell
.\scripts\verify-phase2.ps1
```

Physical/live discovery gate:

```powershell
.\scripts\verify-phase2.ps1 -LiveDiscovery
```

GitHub Actions adds all four Phase 2 crates to the Windows unit-test gate and still performs a full `cargo check --workspace` plus Svelte check/build.

## Deferred to Phase 3

- service-generated driver installation plans;
- WUA download/install callbacks;
- restore points and targeted driver backup;
- reboot/pending-update coordination;
- post-install verification and rollback/recovery UI.
