# Startup & Background Services Manager

## Purpose

Phase 5 adds conservative boot/startup management without turning AetherCore into an autonomous optimizer. Inventory and recommendations are passive; only explicitly selected, service-issued targets may enter an immutable mutation plan.

## Inventory sources

The Windows implementation enumerates four target families:

1. **Registry startup values** — HKLM Run/RunOnce and loaded user-hive Run/RunOnce locations, inspecting both 64-bit and 32-bit registry views. Only string/expand-string values are eligible for reversible management.
2. **Startup folders** — common and loaded-user Startup folders. Reparse entries and oversized files are observation-only; manageable files carry exact size, modified-time, and SHA-256 evidence.
3. **Scheduled tasks** — enabled tasks with Boot or Logon triggers. Microsoft/Windows task namespaces and security-sensitive tasks are protected.
4. **Background services** — conservatively eligible automatic, own-process third-party/unknown services. Shared-process, driver, launch-protected, essential, dependency-sensitive, Windows, security, networking/VPN, storage, input, and accessibility roles are protected.

The scanner is observation-only. A scan never calls a mutation API.

## Recommendation model

```text
Unreviewed      -> no action
KeepEnabled     -> no action
Disable         -> eligible for plan construction only after service policy checks
```

`Unreviewed` is the initial state for every item after every scan. No implicit selection, “recommended changes” checkbox, or Optimize-All path exists.

Performance evidence is intentionally conservative. Phase 5 does not derive undocumented Task Manager-style High/Medium/Low values. If AetherCore cannot bind trustworthy performance evidence to the exact item, the impact is `Unknown` and recommendation confidence is `InsufficientEvidence`.

## Immutable plan construction

The UI sends only service-issued `item_id` plus a typed decision. It cannot submit registry paths, service names, task paths, executable paths, or raw desired states.

The service:

1. resolves the item in the current scan snapshot;
2. rejects stale scan/epoch/item IDs;
3. ignores `Unreviewed` and `KeepEnabled`;
4. rejects protected or unmanageable targets;
5. requires the additional service-change confirmation when any service is selected;
6. freezes the exact original state and typed intended state into `PlanAction::ChangeStartupTarget`;
7. creates the canonical digest and requires the existing one-shot UAC authorization flow.

An empty action set returns `PassiveDefault`; it does not create a mutation plan.

## Reversible state model

### Registry Run/RunOnce

Frozen evidence includes hive, logical key, value name, registry view, value type, and raw value bytes. Disable deletes only that exact value after drift checking. Restore writes the original type and bytes back after checking that the expected disabled state still exists.

### Startup folder

Frozen evidence includes original path, expected file size/mtime/SHA-256, and the service-owned backup destination. Disable moves the file to the AetherCore backup root; it is not deleted. Restore moves the same preserved file back only when both source/destination evidence are compatible with the expected state.

### Scheduled task

Frozen evidence includes task path, Enabled state, and a normalized task-definition hash. Only the task-level `Settings/Enabled` element is removed from identity comparison; trigger-level Enabled fields and every other task-definition field remain hashed, so unrelated scheduling drift is still detected. Disable/restore changes only the task Enabled property.

### Service

Frozen evidence includes service name, start type, delayed-auto state, service type, binary path, and launch-protected signal. AetherCore does not stop the service. The Phase 5 disable target is Demand/Manual start, not `SERVICE_DISABLED`; restore reapplies the exact original start type and delayed-auto setting.

## Protection policy

Protection is enforced in the privileged service/platform layer, not merely in the UI. The current policy fail-closes for:

- known essential Windows service identities;
- Microsoft/System32/svchost-hosted services;
- launch-protected services;
- service drivers/shared-process classes outside the narrow manageable set;
- services with detected reverse dependencies;
- security/antivirus/EDR/firewall terms;
- networking, Ethernet, Wi-Fi/WLAN and VPN roles;
- storage, disk, NVMe and RAID roles;
- keyboard, mouse, touchpad and other input roles;
- accessibility, screen-reader and Braille roles;
- Microsoft Windows scheduled-task namespaces and security-sensitive task definitions.

Ambiguous targets are observable but not manageable.

## Durable journal and recovery

Before any mutation, the service persists a `StartupChangeRecord` with:

- independent `change_id`;
- optional `origin_change_id` for restores;
- plan/item/kind metadata;
- exact original and applied state JSON;
- state/detail timestamps.

Restore never rewrites history. It creates a new immutable plan and a new change record linked to the original disable. On successful restore, the original is marked `Restored`, while the restore change remains an auditable record of its own.

On service restart, AetherCore never replays a startup mutation. Prepared changes are reconciled by re-reading native state:

- exact original state -> `NoChange`;
- exact intended target -> `AppliedRecovered`;
- any other/unknown state -> `RecoveryRequired`.

An interrupted executing plan is marked failed/recovery-required rather than automatically applying another change.

## Mutation serialization

Phase 5 shares AetherCore's machine-wide mutation lock with driver installation, System Repair, and Cleanup. One privileged mutation family owns the barrier at a time.

## UI contract

The Startup screen provides:

- source/kind/protection/evidence visibility;
- explicit Unreviewed / Keep Enabled / Disable choices;
- no implicit optimization action;
- an extra confirmation for service changes;
- immutable-plan review + UAC;
- execution state and failure/recovery details;
- durable change history;
- Restore Original only for eligible applied disable records.

The UI never receives authority to mutate arbitrary native objects.
