# AetherCore server readiness assessment

Phase 37 Stage 4. Every claim below is followed by the evidence for it, or by
the reason it could not be established. Measurements were taken on the Phase 36
qualification VM: Parallels Windows 11 Pro ARM64, build **26200**, **4 logical
CPUs, 9,657,057,280 B RAM**, with AetherCore 0.1.5/0.1.6 installed from the MSI.

## Windows Server implementation update (feat/windows-server)

The pre-change measurements below remain the historical baseline. The live
source now admits Windows 11 clients (build 22621+) and Windows Server 2019,
2022, or 2025+ member servers (build 17763+) in both
`installer/wix/Product.wxs` and `installer/wix/Bundle.wxs`; domain controllers
are explicitly refused by policy, and older Server builds remain refused.
`WINDOWSINSTALLATIONTYPE=Server Core` levels out the desktop feature
and Start Menu component in the MSI, and Burn skips the WebView2 prerequisite.

The SKU-aware matrix in `crates/platform-capabilities` classifies the documented
`ProductType`/`InstallationType` registry values and reports `windowsServer` or
`windowsServerCore`. On Server, thermal/power, Game Mode and restore points are
`NotAvailable`; Windows Update/WUA is `Degraded` with a WSUS-policy note; DISM,
SFC, PnP/driver inventory, local intelligence and service/CLI operation remain
available. Server Core additionally reports autonomous idle scheduling as
degraded because `crates/idle-scheduler/src/windows_state.rs:106-110` requires an
active console session. `crates/security-audit/src/lib.rs` uses the same label,
so Windows compliance reports no longer claim the platform is `other`.

These are hermetic/source-level changes only. No Windows Server SKU is present
in this environment, so installation, WSUS, PnP, restore-point and Server Core
runtime qualification remain open; execute the checklist in
`docs/WINDOWS_SERVER_SUPPORT.md` on disposable Server 2019, 2022, or 2025+
Evaluation VMs.

---

## 4a. What breaks on Windows Server, versus Windows 11 client

### The installer admits supported member servers and refuses domain controllers

Read out of the **built package**, not the source, via the MSI `LaunchCondition`
table:

```
LAUNCH_CONDITION = VersionNT64 AND ((MsiNTProductType = 1 AND OSCURRENTBUILD >= 22621) OR (MsiNTProductType = 3 AND OSCURRENTBUILD >= 17763))
MESSAGE          = AetherCore supports 64-bit Windows 11 build 22621+ and Windows Server 2019, 2022, or 2025+ (build 17763+).
DC_CONDITION     = MsiNTProductType <> 2
DC_MESSAGE       = AetherCore does not support domain controllers.
```

`MsiNTProductType` is 1 for a workstation, 3 for a member server, and 2 for a
domain controller. The package admits supported member servers and has an
explicit domain-controller refusal policy.

This host reads `Win32_OperatingSystem.ProductType = 1`, `Caption = Microsoft
Windows 11 Pro`, `CurrentBuildNumber = 26200` — a workstation, which is why it
installs here.

The build floor compounds it. `OSCURRENTBUILD >= 22621` is checked against
`HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\CurrentBuildNumber`:

| SKU | ProductType | CurrentBuildNumber | passes today? |
|---|---|---|---|
| Windows 11 22H2+ client | 1 | 22621+ | **yes** |
| Windows Server 2019 | 3 | 17763 | **yes** |
| Windows Server 2022 | 3 | 20348 | **yes** |
| Windows Server 2025+ | 3 | 26100+ | **yes** |
| a domain controller | 2 | any | no |

The server floor is deliberately 17763, covering Server 2019, 2022, and 2025+;
Server 2016 is not in the supported set.

### Server Core (no GUI)

- `aethercore-desktop.exe` is a Tauri 2 application and requires the Evergreen
  WebView2 runtime, which `Bundle.wxs` installs as a chained prerequisite.
  Server Core has no shell to host it. The desktop component cannot work there.
- The service and `aetherctl` do not depend on WebView2 or on any window.
  Everything measured in section 4b was produced with **no AetherCore GUI
  process running at all**.
- The MSI installs the desktop binary and its Start Menu shortcut
  unconditionally. There is no feature selection that omits them, so a Server
  Core install would today place an unusable executable on disk. Not fatal,
  but it is a real packaging gap for that SKU.

### Client-only APIs — searched for, and mostly not found

- `crates/system-repair` uses the **DISM API** and `dism.exe`. Both are present
  on Server; this is not a client-only dependency.
- `crates/startup-manager` uses `ITaskService` to read and toggle the user's
  existing startup items. Present on Server.
- No `RegisterTaskDefinition` anywhere: the product creates no scheduled tasks.
- No firewall rule creation anywhere; the firewall lane reads configuration
  files only.
- `SHGetKnownFolderPath(FOLDERID_ProgramData)` — present on Server.

**One concrete defect that is not SKU-specific but shows up here first:**
`crates/security-audit/src/lib.rs:110` `platform_tag()` tests only `macos` and
`linux` and returns `"other"` for everything else. Every compliance report
produced on Windows — the product's primary platform — therefore carries
`host_fingerprint = "other:<digest>"`. Observed in the Gate S1 and Gate S3
reports. A fleet of compliance evidence that cannot name its own platform is a
poor foundation for a server audit trail.

---

## 4b. Headless operation — proven

The whole of Gates S1, S2 and S3 was executed with no interactive AetherCore
process anywhere:

```
SERVICE_PID=6880
SERVICE_SESSION_ID=0        (0 = the non-interactive services session)
CALLER_WHOAMI=nt authority\system
CALLER_SESSION_ID=0
```

and the full process list on the box shows exactly one AetherCore process:

```
aethercore-maintenance-service   pid=6880   ws=81,571,840   cpu_s=11.14
```

Every verb in Gate S1 (nine verbs × two token contexts, 18/18 returned), every
verb in Gate S3, and every fleet verb in this stage ran through a service in
session 0 driven by clients in session 0. The desktop application was never
started.

**Stated limit, because it is not the same claim:** this machine does have a
console session (`console  hasanalaaa  id 2  Active`). Nothing measured
depended on it — no AetherCore process ran in it — but "service plus CLI with
literally no logged-on user" was not tested, because logging off the console
session of the qualification VM requires a human at the Parallels console to
restore it. What is proven is *session-0-only operation*; what is not proven is
*zero-session operation*.

---

## 4c. The fleet surface, exercised against more than one target

Two host records were created and driven for real through the installed CLI:

```
fleet add --id h1 --name "Target One" --host 10.77.0.11 --user svcaudit
fleet add --id h2 --name "Target Two" --host 10.77.0.12 --user svcaudit --tag lab

fleet list  -> both hosts, "trusted": false
fleet show --id h1 -> {"auth":"agent", ... "schema":"aethercore.fleet.host.v1"}

fleet probe --host h1 --host h2
  -> {"hosts":[
       {"hostId":"h1","outcome":"not_verified","detail":"host is not authorized in the AetherCore trust store"},
       {"hostId":"h2","outcome":"not_verified","detail":"host is not authorized in the AetherCore trust store"}]}

fleet audit --profile cis-l1 --host h1 --host h2   -> same per-host typed outcome
```

**What that does and does not prove.** It proves the multi-host path is real:
scope resolution, per-host isolation, deterministic ordering, and a typed
outcome per host. It proves the trust store **fails closed** — neither host was
contacted, because neither is pinned. It does **not** prove a successful remote
audit, because there is no second machine.

`ssh.exe` is present (`C:\WINDOWS\System32\OpenSSH\ssh.exe`,
`OpenSSH_for_Windows_9.5p2`), so the transport could run; the missing piece is a
target, not a capability.

Hermetically, `crates/fleet` passes **59 tests** (47 unit + 12 integration),
including `gd4_multi_host_orchestration_hermetic`, `concurrency_is_bounded`,
`batch_is_deterministic_and_isolated`, `trust_1..trust_5` admission and
revocation, and `cf1_private_key_material_never_enters_trust_storage`.

**No fleet result over real SSH to a second machine is claimed. That remains
unqualified.**

### Two fleet defects found by running it, both fixed

1. **The scheduler could not read back what it wrote.**
   `fleet schedule add` returned 0 and `fleet schedule list` showed the
   schedule, but `fleet schedule run-due` returned **exit 5,
   `fleet.schedulesInvalid: missing field \`schema\``**. The writer hand-built
   its JSON and omitted `schema`, which `FleetSchedule` declares with no serde
   default under `deny_unknown_fields`. The scheduled-compliance capability —
   the entire point of the fleet surface on a server — was broken end to end on
   a real install while eight `scheduler_runner` unit tests passed, because they
   construct `FleetSchedule` values directly and never traverse the CLI writer.
   Fixed by serializing the typed value that was already built and discarded;
   `run-due` now returns `{"ran":0,"runs":[]}` with exit 0.

2. **Fleet state was per-user, and survived uninstall.** See 4d/§16.9 —
   `%APPDATA%\aethercore` held the inventory, the schedules and the **SSH trust
   store**, so an administrator's fleet was invisible to a scheduled run under
   the service account, and a clean uninstall left the trust store behind. Moved
   under `%ProgramData%\AetherCore`.

3. **The entire fleet surface was missing from `aetherctl --help`.** The server
   capability was undiscoverable from the tool. Added.

---

## 4d. Resource cost — real numbers

Sampled once per second from the live service process, with the embedded model
**active** (`insights list` reports `engineLabel: "localModel"`).

| phase | window | CPU | working set (avg / max) | private commit | handles | threads |
|---|---|---|---|---|---|---|
| **idle** — no client, no scan | 60 s | 0.297 CPU-seconds = **0.50 % of one core** (0.12 % of this 4-core box) | 68.8 MB / 69.2 MB | 54.5 MB | 185 | 8 |
| **scanning** | 60 s | 0.766 CPU-seconds = **1.28 % of one core** | 80.7 MB / 81.2 MB | 57.7 MB | 335 | 14 |

The scan was doing real work during that window: 205 facts, 61 findings,
2 collectors, rule engine `phase17.1-rules-v2`.

### The number a server operator actually needs to see

```
WorkingSet64        =      81,559,552   (resident, steady state)
PeakWorkingSet64    =   1,132,965,888   (1.13 GB, at model load)
PrivateMemorySize64 =      58,376,192   (private commit)
VirtualMemorySize64 =   5,702,553,600   (address space, includes file mappings)
```

Steady-state residency is **~82 MB** and private commit is **~58 MB** — genuinely
cheap. But loading the 1,117,320,736-byte model at service start touches
**1.13 GB** of physical memory before the working set is trimmed back. The model
is memory-mapped rather than privately committed (private commit stays at 58 MB
while the address space is 5.7 GB), so it is reclaimable under pressure — but
the transient is real and it happens on **every service start**, which for a
delayed-auto-start service means every boot.

On a memory-constrained or densely-packed server this is the figure that
matters, not the 82 MB steady state. Nothing in the product currently lets an
operator run the service without the model.

### Disk

| | bytes |
|---|---|
| `C:\Program Files\AetherCore` | 1,139,623,893 (**1.14 GB**, of which 1.07 GB is the model) |
| `C:\ProgramData\AetherCore` after a scan | 794,976 |
| the MSI itself | 1,099,640,832 |
| the CLI-only archive, for comparison | 1,892,681 |

---

## Verdict

### Works, with evidence

- Fully headless operation in session 0: service plus CLI, no GUI process, no
  desktop dependency (4b).
- Cheap at idle: 0.5 % of one core, ~82 MB resident, ~58 MB private (4d).
- Multi-host fleet orchestration with per-host typed outcomes and a trust store
  that fails closed before any connection is opened (4c).
- Scheduled compliance now round-trips and executes (4c, after the fix).
- 59 hermetic fleet tests, including multi-host orchestration, bounded
  concurrency and trust admission/revocation.

### Does not work

- **Installation on any Windows Server SKU** — refused by the authored launch
  condition, `MsiNTProductType = 1` (4a). This is the single blocking item.
- **The desktop component on Server Core** — needs WebView2 and a shell, and
  the MSI installs it unconditionally with no feature to omit it (4a).
- **`platform_tag()` reports `"other"` on Windows**, so every compliance report
  produced on the product's primary platform is fingerprinted as an unknown
  platform (4a).

### Remains unqualified — say so plainly

- No Windows Server SKU of any version has been executed against. Every 4a
  statement about Server is derived from the authored conditions and published
  build numbers, not from a run.
- No real SSH audit against a second machine. Only the fail-closed path and the
  hermetic orchestration are proven.
- Zero-session operation (no logged-on user at all) — only session-0 operation
  is proven (4b).
- x86_64 hardware. Everything here is ARM64 in a VM.
- Long-run behaviour. The longest continuous observation is minutes; no
  soak, no memory-growth measurement over days, and no measurement of the
  1.13 GB load transient under memory pressure.
