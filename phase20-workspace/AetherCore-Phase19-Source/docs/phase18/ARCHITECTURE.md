# Phase 18 — Global Driver Authority Architecture

Status: `PHASE_18_GLOBAL_DRIVER_AUTHORITY_SOURCE_COMPLETE` (source boundary only; Windows-native qualification pending).

## Architecture

`driver-hub` remains the user-facing discovery coordinator. It now normalizes PnP inventory into `DeviceIdentity`, collects privacy-minimized `MachineProfile`, normalizes provider candidates into `DriverCandidateV2`, and delegates policy to `DriverAuthorityEngine`. Provider/network code, authority ranking, privileged installation, and UI presentation remain separate.

Flow: Device Identity → Machine/OEM Context → Provider Discovery → Candidate Normalization → Trust/Applicability → Authority Ranking → Recommendation → User Selection → Acquisition (where supported) → Sealed Plan → Recovery Preparation → MutationSupervisor → Install → Verification.

## Security boundary

The renderer cannot supply an executable path or arbitrary URL to the privileged installer. Current production-selectable candidates are exact Windows Update offers resolved by the service. `DirectTrusted` is implemented as a non-elevated acquisition/trust foundation but deliberately not exposed as selectable until a provider-specific privileged executor is natively qualified. Firmware remains protected from generic batch execution.

## Truth boundary

`CompleteForKnownProviders` means the configured provider set completed; it does not mean universal global coverage. Offline/partial/provider-unavailable states cannot produce an “Up to date” claim. Windows Update discovery preserves `WU_E_NO_CONNECTION` as typed `Offline` coverage rather than collapsing it into a generic provider failure, so inventory remains usable while update truth remains unknown.
