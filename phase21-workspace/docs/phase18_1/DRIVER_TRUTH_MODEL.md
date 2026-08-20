# Phase 18.1 Driver Truth Model

Status: **source-complete; Windows qualification deferred**.

## Invariant

`DriverCandidateV2` means a concrete update offer/evidence. A management mechanism is not a candidate merely because it exists.

The model therefore separates:

- **Concrete update evidence** — an offer/package identity that can participate in candidate ranking.
- **DriverManagementAuthority** — an official discovery/management path such as NVIDIA App, AMD Software, Intel Driver & Support Assistant, OEM support, or a manual official authority.
- **UpdateAvailabilityEvidence** — `UnknownUntilVendorCheck`, `UpdateAvailable`, `NoUpdateReported`, or `ProviderUnavailable`.
- **ManagementAuthorityAvailability** — `Available`, `Installed`, `NotInstalled`, or `ProviderUnavailable`.

A management authority proves an update only when its update state is `UpdateAvailable` **and** it carries a non-empty concrete evidence candidate identity. Utility presence alone never satisfies this predicate.

## Device status truth

A healthy device receives `RecommendedUpdateAvailable` only when the authority engine selected a concrete candidate. Otherwise its update wording derives from device-aware authority coverage:

- `UpToDate` only when all required authorities were sufficiently evaluated.
- `NoUpdateFoundFromCheckedSources` when required manual/partial authority coverage remains.
- `UpdateStatusUnknownOffline`, `ProviderUnavailable`, or `UpdateStatusUnknown` when truth cannot be established.

Missing/problem devices remain missing/problem even when no update offer exists; unknown update state never becomes health proof.

## Summary truth

- `recommended_update_count` counts concrete selectable recommendations only.
- `vendor_managed_update_count` counts only management authorities carrying actual `UpdateAvailable` evidence.
- `management_authority_count` counts management guidance separately.
- `status_unknown_count` includes device states whose required authority coverage is incomplete/unknown.

This prevents both false-positive update counts and false strong-completeness claims.
