# Component Store & Protected System Files Strategy

## Component store

Read-only component-store classification uses the DISM API `DismCheckImageHealth`, mapping the structured image-health state to typed Healthy, Repairable and NonRepairable evidence. This removes dependence on localized DISM console prose for health classification.

Before `RestoreHealth`, the service repeats the DISM API health preflight. If the image became healthy, the sealed repair assumption is stale and mutation is rejected. A non-repairable image escalates rather than repeatedly invoking repair.

`RestoreHealth` is a fixed System32 executable plus fixed argument array. A zero process exit is mutation evidence, not resolution. Post-repair `DismCheckImageHealth` must prove the intended state.

## SFC

SFC uses fixed `/verifyonly` and `/scannow` operations. Console language is not parsed as repair truth. The implementation isolates a bounded CBS `[SR]` evidence reader; insufficient evidence remains `SystemFilesUnknown` or repair-unverified. Live behavior across Windows languages/builds remains qualification debt.

The graph places system-file repair after verified component-store repair whenever component-store corruption is also supported.
