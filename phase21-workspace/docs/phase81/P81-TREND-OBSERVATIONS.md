# P81 storage trend observation correction

Source/fixture qualification, 2026-10-03. Base: `5d5ad42eb5dfc6f71c8d0e4107396295235a6800`.
This closes the concrete source gaps in ASTRA P81-02/03 concerning counter resets,
documented observation time and same-device history. It does not qualify the hardware matrix.

## Behavior and callers

Previously, read errors 2→1 plus an NVMe counter reset could append “No increase in the
reported error counters since the previous scan.” A missing observation could also produce
that reassurance from a different unchanged counter. The comparator now suppresses that
sentence after any reset, one-sided missing counter or malformed reported NVMe counter.
A positive delta from a separately comparable counter remains useful evidence; u128 arithmetic
stays exact. Both sides absent remains unsupported rather than a fabricated measurement.

The only production caller is `diagnostic-engine::compare_with_previous_scan`, reached from
the actual interactive scan coordinator. It now uses the existing scan start/end journal
metadata as a conservative observation interval: the previous scan must be Ready or Partial,
have positive ordered times, and finish strictly before the current scan starts. The current
observation must not precede its start. Journal state, snapshot id and collection time must
agree with the JSON. This is an interval bounding acquisition, not a device-native timestamp.
Overlapping, undated, reversed, future or inconsistent baselines say nothing; they do not fall
back to a reassuring result. Owner-specific persistence lookup remains unchanged.

Counter history additionally requires nonempty canonical serial identity, an available bus
and nonzero equal sizes on both sides. Unknown bus evidence cannot prove this identity.
There must be exactly one matching disk in each snapshot. The shared counter identity helper
uses the existing serial normalization and ATA/SATA family comparison. PhysicalDrive handle
binding itself is unchanged. No wire fields, persistence migrations, dependencies, deadlines,
background collectors or hardware operations were added.

## Red before / green after

Behavioral red controls ran against the original production logic:

- `cargo test --locked -p aethercore-hardware-telemetry --lib counter`: three failures,
  including reset reassurance and missing-bus identity.
- `cargo test --locked -p aethercore-diagnostic-engine --lib actual_scan_only_uses_an_ordered_unambiguous_owner_baseline`:
  real coordinator plus seeded owner DB published growth from a baseline with no start time.
- An additional Unknown/Unknown bus control failed before the explicit unavailable-bus refusal.

Final portable commands used `CARGO_BUILD_JOBS=2` and a separate target directory:

- `cargo test --locked -p aethercore-hardware-telemetry -p aethercore-diagnostic-engine -p aethercore-performance-bottleneck`:
  84 passed in four executed suites (23 engine, six recovery, 39 telemetry, 16 rules).
  Tests include actual coordinator/DB cases for Ready and Partial valid baselines, missing,
  future and reversed times, running/failed snapshots, duplicate identities on each side,
  owner isolation and inconsistent journal metadata; explicit equal/overlapping/clock-rollback
  interval boundaries; resets, missing and malformed counters; independent valid growth;
  existing exact-u128 and replacement controls.
- `cargo clippy --locked -p aethercore-hardware-telemetry -p aethercore-diagnostic-engine -p aethercore-performance-bottleneck --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `python3 scripts/static_validate.py`: 351 checks, no failures; YAML parser unavailable
  in this local Python environment, explicitly unmeasured.
- `python3 scripts/enterprise-adversarial-audit.py`: 88/88 passed.
- `python3 scripts/check-dependency-freeze.py`: APPROVED; no freeze baseline changed.
- Gate contains/module/reader self-tests: passed; all 15 source readers fail closed.
- Source seal: 1609 workspace files + seven root workflow files verified.
  `python3 scripts/test_source_seal.py`: 13/13 controls, including corruption negatives.

Raw portable log SHA256s retained by the coordinator:

| Receipt | SHA256 |
|---|---|
| Counter red | `50221e6f17d323292a170b37cba8a658f843f91f8f7243f83e74b465cd776a20` |
| Coordinator red | `8a207ed775637fa0b46668992c1734fde4db4c0ed9c23552dffa2574acadf928` |
| Unknown bus red | `740d2cb0a3b7948972d0b7d28f74812cbcbca214ac60dd604b4d47d4948c76ef` |
| Final affected tests | `8cda7771475fc07e620dd4599f1ab4866c7e5a6177d92d5675baa5b90222a923` |

## Qualification still required

Exact integrated-head Windows CI is coordinator-owned and pending for this correction.
No native compilation or live storage call was performed in this task. Real SATA/NVMe/USB,
RAID/Storage Spaces, hot-unplug, real prediction failure and service-restart history evidence
remain unqualified, as do Windows 11 ordinary-user/Narrator/Arabic/200% display requirements.
D7 vendor tables remain owner-deferred. This change closes a source defect; it does not close
D31 hardware acceptance or the full P81 installed phase DoD.
