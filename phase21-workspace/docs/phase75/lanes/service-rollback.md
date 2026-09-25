# P75 lane `service-rollback`: evidence

Every Windows verdict here comes from `windows-installer.yml`, job
`bundle-log-acl-probe` (plus `product-code-probe`), on the GitHub
`windows-2025` runner.

| run | head | product code | what it is |
|---|---|---|---|
| `36024443316` | `de265cc` | unchanged (= `main`) | first before-run. `product-code-probe` red; the probe died in its new upgrade section (probe bug, below) |
| `36025198764` | `4b5dc2a` | fixed | first after-run. `product-code-probe` green; the probe died at the same probe bug |
| `@BEFORE_RUN@` | `d209712` | unchanged (= `main`) | **before**: `de265cc` + the probe fix only, on proof-only ref `lane/service-rollback-before` (not for merge) |
| `@AFTER_RUN@` | `bf16bf6` | fixed | **after** |

**The probe bug.** Both first runs stopped at the first Windows Installer
automation call (`OpenDatabase`) with `DISP_E_TYPEMISMATCH`. `$oldMsi` came
from `Join-Path`, so it was a PSObject, and `Type.InvokeMember` hands that to
IDispatch unconverted. `bf16bf6` unwraps every argument. The throw came before
the Summary/gate step, so neither run printed gate lines. Every section before
it ran and was logged, and the rollback lines quoted below are from those logs.

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
column is from run `36025198764`. @RERUN_ROLLBACK@

**Proposed status:** CLOSED by P75. @CLOSE_RUNS@

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

@UPGRADE_TABLE@

The plain uninstall is now gated to leave no data root. It used to be only
recorded.

**Proposed row:** "A major upgrade deleted `%ProgramData%\AetherCore`" —
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
| before, run `36024443316` | both `{0F9F349D-01C8-B3C2-7242-83B5D29047C9}` | both `{0F9F349D-01C8-B3C2-7242-83B5D29047C9}` |
| after, run `@AFTER_PC_RUN@` | @AFTER_PC@ | @AFTER_PC_WIN@ |

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

**Proposed row:** "Every build had one ProductCode (`$input`)". CLOSED by P75,
`product-code-probe`.

---

## Local proof (from `phase21-workspace/`, Mac)

@LOCAL_PROOF@
