# Provider Registry and Coverage Inventory

The authoritative machine-readable inventory is `DRIVER_PROVIDER_COVERAGE.json`.

The registry is local/source-defined. It contains official authority metadata only; it is not a cloud database, package mirror, paid API integration, or driver warehouse.

## Implemented automation

`microsoft.windows-update` is the only source-complete machine-readable automatic discovery path in this phase. It is Windows-managed and does not expose arbitrary package execution.

## OEM registry

The source models Dell, Lenovo, HP, ASUS, Acer, MSI, and Microsoft Surface. Where a stable official machine-readable adapter has not been proven in this environment, the registry declares the authority honestly as manual/official-support/utility-managed rather than inventing an endpoint.

## Component registry

The source models NVIDIA, AMD, Intel, Realtek, Qualcomm, MediaTek, Broadcom, and Synaptics. NVIDIA/AMD/Intel are represented as official management channels with update availability unknown until the vendor authority supplies evidence. Other component vendors remain typed manual official authorities unless a proven safe adapter exists.

## Fail-closed behavior

`ProviderRegistry::validate()` rejects malformed entries and duplicate provider identities. Capability describes what a provider *could* do; device scan evaluation separately records whether that provider was actually evaluated.
