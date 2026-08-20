# Device and Machine Identity

`DeviceIdentity` retains PnP instance identity in-memory plus normalized Hardware IDs, Compatible IDs, class/class GUID, enumerator, manufacturer, installed INF/provider/version/date, problem code, and PCI vendor/device/subsystem/revision fields when those semantics exist. Non-PCI buses remain valid; no PCI assumption is required.

Persistent preference keys use a SHA-256-derived privacy key rather than raw PnP identity. `MachineIdentity` reads only machine-level authority fields from documented Windows registry locations: system manufacturer/product/family/baseboard product and Windows product/build/display version. Serial number, UUID, asset tag, and raw hardware serials are excluded.

`MachineProfile` distinguishes an OEM-like machine only when manufacturer evidence is meaningful; generic firmware placeholder strings are not treated as OEM proof.
