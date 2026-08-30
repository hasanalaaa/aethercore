# Device Authority Coverage Specification

Coverage is evaluated per device, not inferred from one global Windows Update state.

`DeviceAuthorityCoverage` records:

- required authorities
- evaluated authorities
- unavailable authorities
- unsupported authorities
- manual authorities
- completeness

Required authorities are derived from normalized device identity, GPU/vendor context, machine context, and the local provider registry. Irrelevant authorities are not demanded for every device.

## Completeness

The implemented states are:

- `CompleteForRequiredAuthorities`
- `Partial`
- `Offline`
- `ProviderUnavailable`
- `ManualAuthorityRequired`
- `Unknown`

`permits_strong_up_to_date()` returns true only for `CompleteForRequiredAuthorities` when every required authority was actually evaluated. Manual, unavailable, offline, unsupported, partial, or unknown authority coverage withholds the strong claim.

Examples:

- Generic Microsoft-managed component: Windows Update may be the only required authority.
- OEM-specific device: Windows Update plus the applicable OEM authority may be required; if the OEM adapter is manual-only, coverage is not complete.
- NVIDIA/AMD/Intel graphics: Windows Update plus applicable component management authority can be required; management availability is not update evidence.
