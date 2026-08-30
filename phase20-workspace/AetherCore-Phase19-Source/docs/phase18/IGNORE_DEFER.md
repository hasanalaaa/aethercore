# Driver Ignore and Defer Semantics

Phase 18 persists privacy-safe driver authority overrides:

- `IgnoreExactVersion`: suppresses only the exact candidate version for that device privacy key. A later version is not suppressed.
- `RemindLater`: time-bounded defer (default seven days) scoped to the candidate provider/device.
- `IgnoreOptional`: persistent optional-update preference and only accepted for a candidate currently classified Optional.

The preference IPC is principal-bound and snapshot-bound. The renderer sends only scan ID, inventory epoch, candidate ID, and a closed policy string. The service resolves the candidate from the current Ready snapshot, rejects stale scan/epoch/candidate state, persists the privacy-safe override, and republishes the updated snapshot.

There is intentionally no “never update this device” blanket rule in this phase.
