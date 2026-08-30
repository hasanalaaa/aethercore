# Phase 17 Correlation Rules

## P17-CORR-001 — possible driver regression

Inputs: a normalized `DriverChange` fact and a later `Crash` fact. Both observations must not be future-dated, must fall inside the current seven-day window, and the crash must occur after the driver change within seven days.

Output: High severity, Medium confidence, `POSSIBLE_DRIVER_REGRESSION`. Evidence from both facts is attached. The wording is intentionally probabilistic and does not claim root cause.

## P17-CORR-003 — crash with hardware evidence

Inputs: recent `HardwareEvent` and recent `Crash` facts inside seven days.

Output: Critical severity, Medium confidence, `CRASH_WITH_HARDWARE_EVIDENCE`. This indicates a serious co-occurrence requiring hardware review; it does not claim a specific component caused the crash.

## Deliberately not implemented

General Windows Update failure + component-store corruption is not correlated in Phase 17 because the existing `update-engine` integrated here is the signed AetherCore self-update subsystem. A future Windows-servicing fact source may enable that rule without conflating unrelated update domains.
