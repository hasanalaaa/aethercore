# P83 boot producer checkpoint

This source checkpoint extends the actual crash-diagnostics boot collector and its existing snapshot caller. It does not close the Startup display, current-boot baseline, attribution-to-inventory or boot finding tasks.

## Authority and measured fields

ASTRA-PLAN.md P83-01A/01B/05B/06B and D6/D10 define the bounded read-only scope. Actual Windows 11 Pro build26200 publisher metadata SHA-256 `cb0d14b5f9deff0595996f1fbc5edc4e665e1850b342d84f4246512410aaa9ac` proves Diagnostics-Performance event100/SystemVersion2 and payload BootTsVersion2, StartTime/EndTime FILETIME, positive SystemBootInstance UInt32 and measured BootTime milliseconds; event101/103/SystemVersion1 named StartTime/Path/TotalTime/DegradationTime; Kernel-Boot event27/SystemVersion1 UInt32 BootType. No cold/restart/fast-startup numeric semantics are inferred. Raw token0 is a Windows-recorded class, without an invented cold label.

Authorized separate local event receipt SHA-256 `9c2a8eefa7437b27afb4e97e62c3568f1afccbeed87f2bccd93f8b8ad82dbd75` contains four event100v2 boots, each with exactly one raw0 kernel event inside its valid nonoverlapping start/end interval. Event101 StartTime equality is exact. No actual event103 record was captured. That receipt contains private owner paths and is not committed. The native latest-three duration median would be28545ms; four total observations cannot qualify a five-prior baseline. This is metadata/history evidence, not execution of the new producer on Windows.

## Actual implementation

- `crates/crash-diagnostics/src/windows_impl.rs::render_system` retains raw FILETIME ticks and independently reads SystemVersion as a strict scalar UInt8 at the documented index17. Existing crash categories remain unchanged.
- `collect_boots` reads named event100 fields, Kernel-Boot27 on System and event101/103 on Diagnostics-Performance/Operational under the existing single CollectorControl, isolation gate, cancellation and deadline. Caps are64/64/128 scanned records,256KiB per render and512KiB retained field text per query. At most20 boots and32 delays per boot are delivered; the native total delay input is additionally bounded by128. No channel is enabled, no extra Startup query or startup/crash dependency is introduced.
- `boot::boot_from_event` pins provider/event/SystemVersion independently from BootTsVersion, requires positive supported boot identity/duration plus a valid completed interval before qualification, and preserves raw100ns FILETIME. It never substitutes interval subtraction for BootTime: actual observed interval spans about110seconds while BootTime is25414ms.
- `bind_boot_evidence` checks duplicate identity and interval overlap before dedupe could hide them. A complete supported kernel read with exactly one event inside the boot interval supplies a fixed-provider/event27/version1 raw class token. Missing, malformed, unsupported, capped or conflicting reads produce no class. Missing secondary channels leave duration history with unknown class/attribution. The final cancellation checkpoint prevents publishing after cancellation even if a boundary returned records.
- Delay parsing pins event101/103 version1, exact StartTime, bounded absolute full drive path and measured UInt32 times. Exact100ns start equality and unique full-path/event identity are required. Relative/dot/UNC/environment paths and basename/display-name guesses remain unattributed; there are no filesystem path probes. Event103 is only a measured service **binary** subject; Name is not treated as an SCM key. Private paths stay in the internal local evidence; export redaction and owner-scoped consumer wiring remain required at the next checkpoint.
- Added BootEvidence fields use serde defaults: an old snapshot does not acquire a completion proof, class or delays. Raw FILETIME stays Rust u64; it must never be forwarded through a lossy JavaScript number if a later consumer needs exact identity. Existing diagnostic-engine consumer still maps the old historical duration fields and compiles/tests unchanged.

## Verification

All Rust commands used jobs2 with an isolated target directory. No native Windows build or live Event Log invocation was performed for this commit.

- Initial new typed-parser/controller fixtures were red because the production APIs did not exist; no acceptance claim comes from that compile failure.
- An actual malformed completion-flag fixture reproduced a production unwrap panic before correction. It now safely leaves class/attribution unknown, with no panic shortcut or gate weakening.
- `cargo test -p aethercore-crash-diagnostics --all-targets`: **27 passed**. Controls cover exact event schema, completed positive identity, unsupported/future/reversed/missing fields, one-tick alias, unique kernel binding, duplicates/overlap, path/time parsing and attribution cap.
- `python3 scripts/test_boot_collection_flow.py`: actual production `collect_boots` and parser compiled unchanged with std-only read boundaries; **6 passed**. Two temporary copied-body negative controls each fail exactly one required test: missing completion fence and ignoring incomplete primary history. They do not edit production or replace the parser with a model. Integrator must include this new script in native/CI gate self-tests.
- Host and WindowsGNU `cargo clippy -p aethercore-crash-diagnostics --all-targets -- -D warnings` (GNU includes `--target x86_64-pc-windows-gnu`): passed. This cross-compiles actual bindings and the scalar-version/FileTime fixture; it does not execute the native fixture.
- `cargo test -p aethercore-diagnostic-engine --all-targets`: **23 unit +8 measurement integration passed**.
- Static351 checks failed=[], **parse_yaml unmeasured** (local PyYAML absent); Zenith35/35 and Enterprise88/88 passed. `cargo fmt --all --check` and `git diff --check` passed. Delivered-file manifest is regenerated and independently verified before commit.

Raw receipt identifiers and SHA-256 (external local logs, portable evidence names):

- `aethercore-p83-boot-producer-red.log`: `9fe79a511e977fe729b1547fc96729bd5a6337d69c470ca120742591dec709f2`
- `aethercore-p83-boot-interval-red.log`: `31d585578f525ea63c0e620fe333aa37cd23fcb966c30625863638847bf9bd8c`
- `aethercore-p83-boot-producer-green.log`: `5080d316b88ef9f2a4b859e0ed08cdfba1263b1aa141a7d332240144380cbf3e`
- `aethercore-p83-boot-flow-green.log`: `7b96e278fd13a9fc5fbe452999b9e72e1be607ed9e756db8c6554db894bb94a4`
- `aethercore-p83-boot-cross-clippy.log`: `951c286c57874ffc1ab9a40409f6b50c020a94fea29694980b2c0bb22fe4c6ec`
- `aethercore-p83-boot-host-clippy.log`: `76bb6a4006acb402d719fa153610e111149f407800e90dbe26df2b8243dbd71a`
- `aethercore-p83-boot-consumer.log`: `51e707c7d9bd18b966945e404939d9e3cdb89db24ce7b78a01450552a1d68a41`
- `aethercore-p83-boot-static.log`: `a5d744073a24956cfbcb8689961b00b6e5f73a4f6c4930cc049ae985c8c531ba`
- `aethercore-p83-boot-zenith.log`: `a620db988112f342e7bbe790e66a040d884975a1b8f7b795a335913f0b20084e`
- `aethercore-p83-boot-enterprise.log`: `ad087999f7106228452af8ff2104a574dab9e774df9423a7544ab58cf0511081`

## Remaining qualification and consumer checkpoints

Wire/protocol/UI median count/window, exact inventory join and current-boot proof are not delivered here. Current baseline requires five older same-token completed boots and a separately trusted OS-start reference matching measured boot identity within its documented precision; latest historical row or a fresh scan is insufficient. No current regression, resolution, startup disabling or causal saved-time statement is introduced. Device/native acceptance, unsupported build variation and service-subject runtime evidence remain NOT RUN; D7/D9/D11/D12 remain outside scope. Root owns ledger, integration and full native CI.
