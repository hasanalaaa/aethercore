# P75 lane `no-egress`: no Windows Update traffic at rest

Branch `lane/no-egress`, base `9747fe3`. The invariant under repair: no network,
account, login or telemetry at rest.

## 1. The defect, reproduced

Read path at base `9747fe3`, then a failing test.

1. `services/maintenance-service/src/main.rs:143` always starts the idle scheduler (`scheduler::start`).
2. `crates/idle-scheduler/src/policy.rs:27-30` makes `DriverDiscovery` eligible every 12 h of idle time.
   `model.rs` marks it `network_sensitive`, so it only defers on a metered or unknown network. It still runs.
3. `services/maintenance-service/src/scheduler.rs:102-106` runs it as `driver_hub.passive_scan_with_fence`.
4. `crates/driver-hub/src/lib.rs:702` (base): the passive scan called `backend.updates()`. That is the same
   call the user's own scan makes (`:818`).
5. `crates/windows-update/src/windows_impl.rs:139` (base): `searcher.SetOnline(VARIANT_BOOL(-1))`, which is an
   online search against Microsoft Update or the configured WSUS.

So an idle machine contacted Microsoft Update about every 12 h. Nobody clicked anything and nothing asked for consent.

**Red test on unchanged code** (base plus the test only):

```
$ cargo test -p aethercore-driver-hub --locked passive_scan_never
test tests::passive_scan_never_reaches_an_online_only_backend ... FAILED
assertion `left == right` failed: the passive scan went online
  left: 1
 right: 0
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 19 filtered out
```

## 2. The fix

- **`crates/windows-update`**: new `SearchScope::{LocalCacheOnly, Online}`.
  `discover_driver_offers(scope)` sets `IUpdateSearcher.Online` explicitly to
  `VARIANT_FALSE` or `VARIANT_TRUE`; it no longer relies on a default.
  - The only callers are driver-hub and this crate's own ignored live test.
  - No signature used by `driver-install`, `system-repair` or `driver-authority` changed.
  - `probe_update_health` and `execute_driver_updates` are untouched.
- **`crates/driver-hub`**: re-exports `SearchScope`, and `DiscoveryBackend` gains `updates_in(scope)`.
  - The hub calls only `updates_in`. `passive_scan_with_fence` names `LocalCacheOnly`. The user's scan
    (`start_scan_with_lease` → `run_scan`) names `Online`.
  - The default `updates_in` **fails closed**: `LocalCacheOnly` returns `Unavailable` and never calls the
    online `updates()`. A backend written without a cache search, like the existing
    `driver-install/tests/coordinator.rs` fake, can never be sent online by a passive path. That fake
    compiles and passes unchanged.
  - `WindowsDiscoveryBackend` overrides `updates_in` to pass the scope through to WUA.
- **Truth follow-on** (same change, required so the fix does not overclaim): a cached answer is only what
  Windows last synchronised. An empty cache on a machine that never scanned would otherwise read as "up to date".
  - `windows_update_scan_state` now takes the scope. A `LocalCacheOnly` result can reach at most
    `Partial`, never `CompleteForRequiredAuthorities`.
  - A passive snapshot therefore shows `NoUpdateFoundFromCheckedSources` instead of `UpToDate`. The next
    user scan restores the full verdict.
  - No new user-visible string: `Partial` and its status are already in both catalogs.
- One `#[allow(clippy::too_many_arguments)]` on the private `match_inventory_with_overrides` (8 args). This
  follows the workspace convention (10 existing sites, for example `system-repair/src/lib.rs:1111`). Changing
  the signature of the public `match_inventory` is not warranted.

## 3. What `Online = false` guarantees (Microsoft's documentation)

- **[MS-UAMG] §3.38.4.13, `IUpdateSearcher::Online` (Opnum 22).** The setter's value is `VARIANT_TRUE` if
  the search may contact the server, or `VARIANT_FALSE` "if it uses local data only".
  https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-uamg/31a1923c-fb3a-416c-825d-168a79ef5c95
- **[MS-UAMG] §3.38.4.11, `IUpdateSearcher::Search` (Opnum 20).** Going online is conditioned on
  the Online element being `VARIANT_TRUE` (a SHOULD). The same section says the agent MAY self-upgrade only when
  `CanAutomaticallyUpgradeService` is `VARIANT_TRUE`.
  https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-uamg/8bc81f1d-0491-463c-a347-a4486b7d83ab
- **[MS-UAMG] Appendix B.** Unless a note says otherwise, SHOULD implies Windows follows it. No note in
  Appendix B concerns `Online` or `Search`'s online behaviour; note 41 only says Windows implements Search via a
  local-only COM interface.
  https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-uamg/50faf1ae-d0ab-42cb-87e2-33f30391e807
- **Win32 `IUpdateSearcher::put_CanAutomaticallyUpgradeService`.** The only currently valid value is the one
  that does not upgrade WUA, and AetherCore never sets it.
  https://learn.microsoft.com/en-us/windows/win32/api/wuapi/nf-wuapi-iupdatesearcher-put_canautomaticallyupgradeservice
- The Win32 `put_Online` page only restates the property ("whether the UpdateSearcher goes online"). It gives
  no stronger guarantee.

Decision: offline search is a documented protocol guarantee ("local data only"), so the passive scan keeps WU
discovery in `LocalCacheOnly` rather than skipping it.

**Proof level, stated honestly:**
- *Measured here:* the scope is propagated. The passive scan requests `LocalCacheOnly`, the user scan requests
  `Online`, and a backend without a cache search is never called online. See the tests below.
- *By documentation:* `VARIANT_FALSE` → no server contact.
- *Not measured:* a packet capture on a Windows host during an idle `DriverDiscovery` run. The
  `SearchScope` → `VARIANT_FALSE` mapping is a two-arm `match` in `windows_impl.rs`. It compiles for the Windows
  target (below) and has no unit test of its own, because such a test would only restate the match.

## 4. Proof

| Command (from `phase21-workspace/`, with `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`) | Result |
|---|---|
| `cargo test -p aethercore-driver-hub --locked passive_scan_never` on base + test | FAILED, `left: 1 right: 0` (§1) |
| Same two tests with the fix but the passive scope flipped to `Online` | both FAILED: `left: [Online] right: [LocalCacheOnly]`, and `left: 1 right: 0` |
| `cargo test -p aethercore-driver-hub -p aethercore-windows-update -p aethercore-driver-install --locked` | driver-hub `21 passed`; windows-update `3 passed`; driver-install lib `6 passed`, `tests/coordinator.rs` `4 passed`; 0 failed |
| `cargo test --workspace --locked` (macOS, at `49370e5`) | exit 0; 135 test binaries, 651 passed, 0 failed |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy -p aethercore-driver-hub -p aethercore-windows-update --all-targets --locked -- -D warnings` | `Finished`, no findings |
| `cargo clippy -p aethercore-windows-update --target x86_64-pc-windows-msvc --all-targets --locked -- -D warnings` | `Finished`, no findings. Covers `windows_impl.rs` and `tests/live_wua.rs` |
| `cargo clippy -p aethercore-driver-hub --target x86_64-pc-windows-msvc ...` | cannot run locally (`libsqlite3-sys` build script: `'stdlib.h' file not found`). The Windows CI job is the compile verdict |
| `python3 scripts/static_validate.py` | `"checks": 347, "failed": []` |
| `python3 scripts/test_gate_readers.py` | `all 14 readers fail closed` |
| `python3 scripts/ps_marker_scan.py` | `total assertions=234 failed=0 unmeasured=3` (UNMEASURED lines are pre-existing, in `verify-installer-security.ps1` and others, none in touched files) |
| `enterprise-adversarial-audit.py`, `phase14-scheduler-audit.py`, `zenith-recursive-audit.py`, `phase18-driver-authority-audit.py` | `failed: []` / PASS |
| `test-phase12-localization.py` | `34/34 checks passed` |
| `phase18_1-driver-truth-audit.py` | FAIL, **pre-existing, identical on base**. `P18.1-TRUTH-003` and `COVERAGE-002/003` look for pre-rustfmt single-line tokens and the pre-P62 `&machine_profile` argument; none of those tokens exists in base or branch. `MACHINE-001` is in `driver-authority` |
| `phase19-windows-repair-audit.py` | FAIL, **pre-existing**: `P19-PLAN-002` concerns `crates/system-repair/src/lib.rs`, untouched |

The new tests are in `crates/driver-hub/src/lib.rs`:
- `passive_scan_never_reaches_an_online_only_backend`: a backend with only the online `updates()`; the passive
  scan makes zero online calls.
- `passive_scan_searches_local_cache_and_user_scan_searches_online`: the recorded scopes are
  `[LocalCacheOnly, Online]`. The passive snapshot is `Partial` / `NoUpdateFoundFromCheckedSources`; the user
  scan is `CompleteForRequiredAuthorities` / `UpToDate`.

## 5. Read-only audit of every other background, idle and startup path

Grep over `crates/ services/ apps/` (Rust and `Cargo.toml`) for: `reqwest ureq hyper WinHttp WinINet Internet
TcpStream UdpSocket std::net Networking WSAStartup Icmp Dns SetOnline IUpdate BITS BackgroundCopy URLDownload
ShellExecute WinVerifyTrust REVOCATION CertGetCertificateChain Invoke-WebRequest curl wget RestoreHealth
LimitAccess INetworkCostManager TcpListener connect( http:// https:// Command::new`.

| Path | When it runs | Network? |
|---|---|---|
| Idle `DriverDiscovery` → `passive_scan` | at rest, every 12 h idle | **was online; fixed** (this lane) |
| Idle `HardwareTelemetry`, `EventLogTriage` (`diagnostic-engine`), `CleanupInventory` (`cleaner`), `StartupInventory` (`startup-manager`) | at rest | no. Their dependency graphs (`collector-runtime`, `hardware-telemetry`, `crash-diagnostics`, `persistence`, `windows-foundation`, `operation-*`) contain none of the tokens above |
| `idle-scheduler/src/windows_state.rs` `INetworkCostManager` | at rest | no. It is a local Network List Manager query |
| Service startup `composition::build`: `UpdateCoordinator::load_with_build` | startup | no. It reads `update-trust.json` from disk; `update-download` is linked only by `apps/desktop` |
| Startup recovery (`driver-install`, `system-repair`, `cleaner`, `startup-manager` `recover_incomplete`) | startup | no. Journal reconciliation only; `system-repair` explicitly never replays DISM/SFC |
| `update-engine/src/platform.rs` `WinVerifyTrust` | update staging | no. It uses `WTD_CACHE_ONLY_URL_RETRIEVAL` (no CRL/OCSP fetch) |
| `windows-update::probe_update_health` (`SetOnline(-1)`) via `system-repair` `update_health_check` | repair **assessment**: `router/repair.rs:15`, `pc-intelligence` deep scan | online, **user-initiated only**. Not at rest |
| `execute_driver_updates` (`execution_windows.rs:277` `SetOnline(-1)`, download) | user-approved driver install | online, user-initiated |
| DISM `/RestoreHealth` (`system-repair/src/windows_impl.rs:383`) | user-approved repair plan | may reach WU, user-initiated |
| Desktop `check_for_updates` / `stage_update` (`HttpsTransport`) | UI button click (`SystemCarePanel.svelte:33`) | online, user-initiated; off unless trust is configured |
| `fleet` transport (`ssh`) | `aetherctl fleet` only | user-initiated CLI |
| UI Drivers page / Deep Scan | button click (`DriversPage.svelte:77,157,162`, `DeepScanPage.svelte:100`) | user-initiated. No scan on mount |

No other at-rest egress was found. Nothing outside this lane's files needed a code fix.

**Findings outside this lane's files (report only):**
- `docs/DRIVER_HUB.md:36` says the searcher is explicitly online. That is now true only of the user's scan.
- `docs/AUTONOMOUS_MAINTENANCE.md:20` and `docs/adr/0016-autonomous-idle-scheduler.md:15` gate `DriverDiscovery`
  on an unmetered network (`model.rs` `network_sensitive`). Passive discovery no longer uses the network, so the
  gate is now over-conservative. It was left in place: removing a gate is a product decision, not this lane's.
- `docs/LOCAL_ONLY.md` §2 lists only the HTTP-client crates as network paths. The OS-mediated WUA paths above
  (user scan, repair assessment probe, driver install) are not listed.
- The workspace `windows` feature list enables `Win32_Networking_WinHttp`, and no Rust source uses it.

## 6. Proposed ledger row (the lead assigns the id)

| `DBT-P75-NNN` | **The idle scheduler's driver discovery ran an online Windows Update search at rest, every ~12 h of idle time, with no user action or opt-in** | **CLOSED by P75 lane `no-egress`: the passive scan searches the local WU datastore only (`IUpdateSearcher.Online = VARIANT_FALSE`); only the user's scan goes online** | Path at base `9747fe3`: `main.rs:143` → `scheduler.rs:102-106` → `driver-hub` `passive_scan_with_fence` → `backend.updates()` (`lib.rs:702`) → `windows_impl.rs:139` `SetOnline(VARIANT_BOOL(-1))`. Red test `passive_scan_never_reaches_an_online_only_backend` fails on base (`left: 1 right: 0`). Fix: `SearchScope::{LocalCacheOnly, Online}` threaded from the passive and interactive entry points; the default `DiscoveryBackend::updates_in` fails closed for `LocalCacheOnly`; a cache-only result is capped at `Partial` coverage, so it never claims `UpToDate`. Guarantee cited from [MS-UAMG] §3.38.4.13 ("local data only") and §3.38.4.11 / Appendix B. **Proof level:** scope propagation measured by two unit tests; the no-traffic effect rests on Microsoft's protocol specification; no packet capture on a Windows host. `docs/phase75/lanes/no-egress.md` | CI (Windows compile); a Windows host for an optional packet capture |
