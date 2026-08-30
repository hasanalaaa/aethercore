# Provider Contract and Inventory

## Contract

`DriverProvider` exposes provider identity, authority class, support predicate, and typed discovery result. Every normalized candidate retains provider provenance, official source, discovery method, publisher, origin, catalog/offer identity, retrieval timestamp, and applicability evidence.

## Implemented provider lanes

| Provider lane | Source implementation | Acquisition/install capability | Notes |
|---|---|---|---|
| Windows Update | Existing WUA collector + `WindowsUpdateProvider::from_offer` | `WindowsManaged` | Exact update ID + revision preserved through plan/install revalidation. |
| NVIDIA | `OfficialUtilityProvider` + existing GPU policy | `OfficialUtility` | NVIDIA App / official NVIDIA support flow; no fake AetherCore direct install. |
| AMD | `OfficialUtilityProvider` + existing GPU policy | `OfficialUtility` | Official AMD software/support flow. |
| Intel | `OfficialUtilityProvider` + existing GPU policy | `OfficialUtility` | Intel Driver & Support Assistant flow. |
| OEM/component adapters | `DriverProvider` framework | Provider-defined | No undocumented endpoint is fabricated. A vendor without a proven machine-readable mechanism remains utility/manual official. |
| Direct package provider | `driver-acquisition` foundation | `DirectTrusted` model only | Production selection remains disabled until provider-specific executor and native trust/staging proof exist. |

There is no AetherCore driver mirror, third-party repository, paid API, or required AetherCore cloud backend.
