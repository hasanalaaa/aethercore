# Windows Update & Services Strategy

Windows Update health is observed through WUA online discovery. An explicit WUA no-connection HRESULT is classified as offline; other failures remain update failures until corroborating evidence exists. This prevents a generic WUA failure from becoming an invented DNS, proxy or component-store root cause.

A failed update discovery may trigger a diagnosis-scoped check of the fixed `wuauserv` service. The only service mutation implemented in this path is the trusted server-side start of that fixed service. Renderer IPC cannot provide a service name or startup configuration.

After a service start, verification requires both the service state and a fresh WUA discovery result. Starting a service alone cannot resolve the Windows Update problem.

The design explicitly excludes blanket service templates, broad cache deletion and `SoftwareDistribution` deletion as routine repair. Component-store or reboot prerequisites are represented as graph dependencies before update retry.
