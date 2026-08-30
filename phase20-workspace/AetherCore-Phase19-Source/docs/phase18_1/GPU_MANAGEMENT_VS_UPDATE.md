# GPU Management vs Update Specification

For NVIDIA, AMD, and Intel graphics, three independent questions are modeled:

1. **Update availability** — is there concrete evidence that an update exists?
2. **Recommended management channel** — which official mechanism is appropriate for checking/managing the device?
3. **Installability** — can AetherCore safely execute the concrete package under current production policy?

An official utility never enters the candidate ranking ladder as though it were a driver package.

Example truthful result:

- Current driver: installed and healthy
- Windows Update offer: none
- Official management: NVIDIA App
- Vendor update evidence: unknown until vendor check
- Device update status: no update found from checked sources / vendor check required by coverage
- Recommended update count: zero

If Windows Update returns a concrete trusted applicable offer, that package can be Recommended while NVIDIA App remains a separate official management option. `DirectTrusted` remains disabled in Phase 18.1.
