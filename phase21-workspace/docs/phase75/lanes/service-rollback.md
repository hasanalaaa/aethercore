# P75 lane `service-rollback`: evidence

Every Windows verdict here comes from `windows-installer.yml`, job
`bundle-log-acl-probe` (plus `product-code-probe`), on the GitHub
`windows-2025` runner.

| run | head | product code | what it is |
|---|---|---|---|
| `36024443316` | `de265cc` | unchanged (= `main`) | first before-run. `product-code-probe` red; the probe died in its new upgrade section (probe bug, below) |
| `36025198764` | `4b5dc2a` | fixed | first after-run. `product-code-probe` green; the probe died at the same probe bug |
| `36184401229` | `618b2c5` | unchanged | **before**: the gate commits plus the repaired harness only, on proof-only ref `lane/service-rollback-before` (not for merge) |
| `36184392149` | `b7a6661` | fixed | **after** (lane head merged with `main` at `bcd7211`) |

**The probe bug.** Both first runs stopped at the first Windows Installer
automation call (`OpenDatabase`) with `DISP_E_TYPEMISMATCH`. `$oldMsi` came
from `Join-Path`, so it was a PSObject, and `Type.InvokeMember` hands that to
IDispatch unconverted. `bf16bf6` unwraps every argument. The throw came before
the Summary/gate step, so neither run printed gate lines. Every section before
it ran and was logged, and the rollback lines quoted below are from those logs.

The unwrap in `bf16bf6` did not fix it. Inside the probe step,
`PSObject.BaseObject` returned a null target (run `36111864600`), and keeping
the wrapper gave `DISP_E_TYPEMISMATCH` again (run `36178393577`). A throwaway
push-triggered workflow (run `36184110472`) ran the same edit as a standalone
script with a plain `InvokeMember` wrapper under pwsh 7.6.6 and PowerShell 5.1
on windows-2025. Both passed and read the new ProductCode back. `0c97599` runs
that script in a child `pwsh` and fails the step if the ProductCode does not
read back. With it, both runs reached every gate.

---

## `DBT-P74-001`: a rolled-back uninstall restores a service that cannot start

**What changed.**
* `apps/install-hardener/src/main.rs`: a third fixed verb, `configure-service`.
  It runs only the SCM steps `apply` already ran: `sc sidtype unrestricted`,
  `sc config start= delayed-auto obj= LocalSystem`, `sc sdset`. They now live
  in one function, `configure_service`, which `apply` calls too, so there is
  one source of truth. System32 tool resolution is one helper, `system32_tool`.
* `installer/wix/Product.wxs`: `RestoreServiceConfigRollback`,
  `Execute="rollback"`, `Impersonate="no"`, `Return="ignore"`,
  `Before="DeleteServices"`, `Condition="REMOVE = &quot;ALL&quot;"`.
* Why not `apply` as the rollback action: it needs both trees present and free
  of reparse points. A rollback can follow a purge that deleted the data tree,
  or one refused because of a junction, which is exactly how the probe forces
  one.
* Why not `MsiServiceConfig`: it cannot restore the `sdset` DACL, and it would
  split the service's settings across two sources.

**The gate was fixed first, in its own commit (`617e560`).** The old gate could
not fail usefully:
* `Save-ServiceState` ran `sc stop` before it sampled `STATE`.
* It rejected only `START_PENDING`.
* It recorded the SID type (`NONE`) and the 251–257 s without gating either.

It now samples state and `sc qsidtype` before any stop, and gates three
things:
* `STATE` is `RUNNING`;
* `SERVICE_SID_TYPE` is `UNRESTRICTED`;
* the forced-rollback uninstall returns in under 60 s.

The install that the rollback uninstalls is gated on its exit code too.

**Rollback order, measured** (MSI log, run `36025198764`, rollback script):
```
16:44:23:564 ActionStart(Name=DeleteServices)  ServiceInstall(Name=AetherCoreMaintenance, … StartType=2 …)
16:44:23:568 ActionStart(Name=RestoreServiceConfigRollback)  CustomActionRollback(Action=RestoreServiceConfigRollback,ActionType=11602,…)
16:44:23:659 ActionStart(Name=StopServices)  ServiceControl(,Name=AetherCoreMaintenance,Action=1,Wait=1,)
16:44:24:708 ActionStart(Name=UnpublishFeatures)
```
The service is re-created, then the action runs (91 ms), then the service is
started, in 1.05 s. On `main` that start took 4 min 05 s.

| forced rollback (junction under the data root) | before | after |
|---|---|---|
| uninstall | `exit 1603 in 252 s` | `exit 1603 in 9 s` |
| `STATE`, sampled before any stop | `1 STOPPED`, Win32 exit 1 | `4 RUNNING`, PID alive |
| `SERVICE_SID_TYPE` | `NONE` | `UNRESTRICTED` |
| `START_TYPE` | `AUTO_START` | `AUTO_START (DELAYED)` |
| `sc stop` | 1062 (not started) | exit 0, 10 ms |

The before column is from run `36024443316` and matches P74's runs. The after
column is from run `36025198764`. Re-measured with the repaired harness, on
the gated summary: before `36184401229`, `exit 1603 in 249 s`, `STATE : 1
STOPPED`, `SERVICE_SID_TYPE: NONE`, `sc stop exit 1062`, and three `GATE:` lines
(249 s over the 60 s limit, not running, SID type not the hardener's). After
`36184392149`: `exit 1603 in 7 s`, `STATE : 4 RUNNING`,
`SERVICE_SID_TYPE: UNRESTRICTED`, process alive, `GATE: pass`.

**Status:** CLOSED by P75. Red `36184401229`, green `36184392149`.

---

## Major upgrade deleted all machine data (new row, audit HIGH, now measured)

**What changed.** `PurgeMachineData`'s condition is now `REMOVE="ALL" AND NOT
UPGRADINGPRODUCTCODE`.

**The measurement.** The probe's last section takes this build's MSI from the
Package Cache. It re-stamps it as `ProductVersion 0.0.1` with a fresh
ProductCode and PackageCode, through the Windows Installer automation API, and
changes nothing else. It installs that with msiexec and writes
`state\p75-upgrade-marker.txt`. Then it runs the bundle, which major-upgrades
it. The old package's own sequence removes the old product (`REMOVE=ALL`,
`UPGRADINGPRODUCTCODE` set).

| | before `36184401229` | after `36184392149` |
|---|---|---|
| earlier 0.0.1 install | exit 0 | exit 0 |
| bundle major upgrade | exit 0 | exit 0 |
| earlier product still registered | no | no |
| `state\p75-upgrade-marker.txt` kept | **no**: `GATE: the major upgrade deleted the machine data under C:\ProgramData\AetherCore` | **yes** (10 entries under the data root) |
| service after the upgrade | not gated in the red run | `RUNNING`, `UNRESTRICTED` |

The plain uninstall is now gated to leave no data root. It used to be only
recorded.

**Row `DBT-P75-009`:** "A major upgrade deleted `%ProgramData%\AetherCore`" —
CLOSED by P75, with the runs above. **Residual:** a package built before this
condition still purges when it is the package being upgraded away. MSI runs
the old package's condition, so this fix cannot reach it.

---

## `build-installer.ps1`: every version had one ProductCode (new row, audit HIGH, now measured)

**What changed.** `Deterministic-ProductCode([string]$input)` →
`([string]$identity)`.

**Why.** `$input` is PowerShell's automatic pipeline enumerator. In the
function body it replaces the bound argument.

**The measurement.** New job `product-code-probe` parses the function out of
the script and evaluates it for `AetherCore/0.1.10/x64` and
`AetherCore/0.1.11/x64`, in both shells.

| | pwsh 7.6.5 | Windows PowerShell |
|---|---|---|
| before, runs `36024443316` and `36184401229` (both `GATE:` lines: two versions share the code) | both `{0F9F349D-01C8-B3C2-7242-83B5D29047C9}` | both `{0F9F349D-01C8-B3C2-7242-83B5D29047C9}` |
| after, run `36184392149` | `{BEC1F014-2C09-FF82-C98F-1B253038AF5B}`, `{37B66DE3-6044-7D75-504F-166B73992F51}` | the same two |

`{0F9F349D-…}` is SHA-256(namespace + empty)[0..15], recomputed in Python. The
fixed values match Python's independent computation:
`{BEC1F014-2C09-FF82-C98F-1B253038AF5B}` and
`{37B66DE3-6044-7D75-504F-166B73992F51}`. The job now fails if two versions
share a code or if the two shells disagree.

**Consequence for machines already installed (inferred, not measured).**
`{0F9F349D-…}` v0.1.11 is registered on the Windows PC (P36, P55, P71 docs).
The version is still 0.1.11, so a build from this branch has a new ProductCode
at the **same** version. `MajorUpgrade` has `AllowSameVersionUpgrades="no"`,
and WiX documents that case as two products side by side, not an upgrade.
Uninstall the old build before installing one from this branch. That
uninstall purges data, as every uninstall does. The next version bump upgrades
normally, but the pre-fix package, when it is upgraded away, still purges
(see the previous section).

**Row `DBT-P75-010`:** "Every build had one ProductCode (`$input`)". CLOSED by P75,
`product-code-probe`.

---

## Local proof (from `phase21-workspace/`, Mac)

* `cargo fmt --all -- --check`: pass.
* `cargo clippy -p aethercore-install-hardener --all-targets --target x86_64-pc-windows-msvc --locked -- -D warnings`: pass.
* `cargo clippy --workspace --all-targets --locked -- -D warnings`: pass.
* `cargo test --workspace --locked`: 137 suites, 653 passed, 0 failed, 1 ignored.
* `static_validate.py` failed `[]`; `test_gate_readers.py` all 14 fail closed; `ps_marker_scan.py`
  234 assertions, 0 failed, 3 unmeasured; `enterprise-adversarial-audit.py` failed `[]`.
* Audits naming touched files: `phase15-security-audit`, `zenith-recursive-audit` pass;
  `phase35-adversarial-audit` fails identically on `origin/main`.

## Not done

`DBT-P74-002` (goal 3 of this lane) was not started.
