# Smart Driver Hub — Phase 2 Design

## Authority and trust

The Smart Driver Hub has two native sources of truth:

1. **Windows PnP manager** for what is currently present and bound on the machine.
2. **Windows Update Agent** for which Windows-managed driver packages are currently applicable according to the machine's configured update source and policy.

AetherCore does not use a proprietary driver database and does not make applicability decisions by comparing version strings.

## PnP inventory model

The inventory is built from SetupAPI/Configuration Manager. Each `DeviceRecord` contains:

- device instance ID;
- display/friendly name and description;
- class and class GUID;
- manufacturer, enumerator, and location;
- Hardware IDs and Compatible IDs;
- raw Configuration Manager status;
- Device Manager problem code, accepted only when Configuration Manager sets `DN_HAS_PROBLEM`;
- missing-driver flag (`DN_HAS_PROBLEM` + problem code 28 / `CM_PROB_FAILED_INSTALL`);
- current driver provider, version, INF path, and date when a bound driver registry key is available.

The scan requests only `DIGCF_PRESENT` devices. The `SWD` software enumerator is filtered from the hardware-focused dashboard. This is intentionally not described as a universal proof that every remaining PnP node is physically discrete hardware; Windows can expose bus/root/virtualized hardware-facing nodes. The product wording therefore uses **present hardware/PnP devices** rather than inventing a stronger guarantee than the platform provides.

## WUA discovery model

The WUA search expression is:

```text
IsInstalled=0 and Type='Driver' and IsHidden=0
```

The searcher is explicitly online. WUA still chooses its configured service, which means enterprise WSUS/Group Policy remains authoritative where configured.

For each update, AetherCore records:

- update GUID and revision;
- Windows title;
- min/max download size;
- driver class;
- hardware ID;
- manufacturer, model, and provider;
- driver date;
- WUA device status/problem number;
- informational support URL.

When `IWindowsDriverUpdate4` is available, its `WindowsDriverUpdateEntries` collection is expanded. This avoids losing device-specific applicability when one WUA update applies to more than one hardware entry.

### Target version honesty

WUA exposes `DriverVerDate`, but `IWindowsDriverUpdate` and `IWindowsDriverUpdateEntry` do not expose a standalone target `DriverVersion` property. AetherCore only displays a target version when the Windows update title ends in a conservative three-to-six-part dotted numeric version. Otherwise the UI renders `Windows offer` and the reported driver date.

No candidate is selected by comparing installed and offered version text. WUA's applicability result is the deciding signal.

## Matching algorithm

The matching engine builds two indexes from the current inventory:

```text
normalized Hardware ID   -> device indexes
normalized Compatible ID -> device indexes
```

Each WUA entry is then resolved in this order:

1. exact normalized Hardware ID;
2. exact normalized Compatible ID;
3. unmatched.

There is no manufacturer/model fuzzy matching and no version heuristic. A WUA entry can legitimately map to multiple current device instances; AetherCore creates a service-issued candidate for each instance. Candidate identity is deterministic over WUA update ID + revision + device instance ID. If the same WUA update exposes both an exact Hardware ID entry and a Compatible ID entry for one device, the candidates collapse to one identity and the exact Hardware ID evidence wins deterministically.

## Scan lifecycle and staleness

The Phase 2 scan lifecycle is intentionally separate from the Phase 1 mutation state machine:

```text
Idle -> InventoryScanning -> UpdateSearching -> Matching -> Ready
                                                \-> Failed
```

The PnP failure is fatal because the physical/current target set cannot be trusted without it. A WUA failure is non-fatal: the inventory still reaches `Ready` with a warning and zero discovered offers, because hardware inspection remains useful offline or when update policy/service is unavailable. A WUA `SucceededWithErrors` result is retained but surfaced as an incomplete-results warning; non-success result codes fail the WUA discovery stage and fall back to the same non-fatal inventory-only behavior. Per-device identity/status reads are part of that same consistency boundary: an unusable device instance ID or a failed `CM_Get_DevNode_Status` call aborts the PnP snapshot rather than fabricating a healthy/default status.

Each run gets a new `scan_id` and `inventory_epoch`. The snapshot is not persisted as durable truth. In Phase 3, any selected candidate must be revalidated against its epoch and current WUA/PnP state before a privileged maintenance plan can be authorized.

## GPU and firmware policy

Every display-class device is protected from automatic selection in the default policy. NVIDIA, AMD, and Intel adapters additionally receive a recognized vendor policy and official companion-app/support action. Unknown/OEM display-class adapters remain visible but locked for manual vendor/OEM review.

Known-vendor detection uses:

- Display class / display class GUID;
- PCI vendor IDs 10DE, 1002, and 8086;
- manufacturer-name fallback.

Known official companion apps are detected from trusted Program Files locations. Detection is advisory: absence means “app not found at the supported machine installation path,” not proof that no vendor software exists anywhere on disk.

Every display-class candidate is visible but locked:

```text
vendor_managed      = true
selectable          = false
selected_by_default = false
```

The UI sends only `nvidia`, `amd`, or `intel` to the Tauri command. The desktop maps that enum to an internal fixed allowlist of official vendor destinations. Arbitrary WUA support URLs never cross into shell execution.

Firmware is a separate protected class. If the present device uses the Windows Firmware setup class, or WUA reports the offer's driver class as `Firmware`, the candidate is still shown but is locked:

```text
firmware_managed    = true
selectable          = false
selected_by_default = false
```

This preserves diagnostic visibility without treating firmware as an ordinary one-click driver update.

## Phase 2 safety properties

- Read-only PnP and WUA discovery only.
- No `IUpdateDownloader`.
- No `IUpdateInstaller`.
- No INF installation API.
- No command shell or PowerShell execution.
- No arbitrary URL shell-open API.
- No driver catalog scraping.
- No fake “latest driver” claim.
- No display-driver auto-selection, including unknown/OEM display vendors.
- Firmware-class offers are visible for diagnosis but are never selectable or preselected; they remain a manual-review surface for a later dedicated policy.
- No persistent interpretation of a stale scan as consent.
