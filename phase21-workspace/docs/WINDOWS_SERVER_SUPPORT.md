# Windows Server support

This is the product shape for Server support before implementation. The live
source remains authoritative; no Windows Server SKU is available in this
environment, so runtime claims below are qualification targets, not results.

## Audit findings

| Surface | Finding (source evidence) | Server decision |
|---|---|---|
| Installer | `installer/wix/Product.wxs` currently requires `MsiNTProductType = 1`; `installer/wix/Bundle.wxs` requires `NTProductType = 1`. Server 2025 can meet the build floor but is rejected by SKU. | Remove the workstation-only predicate. Keep the build floor: Windows 11 client 22621+ and Windows Server 2025+ (26100+). Older Server builds remain refused. |
| Desktop / shell | `apps/desktop` is a Tauri/WebView2 application. `installer/wix/Bundle.wxs` chains the Evergreen WebView2 bootstrapper. Server Core has no GUI/shell. | Desktop feature and Start Menu shortcut are unavailable on Server Core. Service and `aetherctl` remain the complete product. Burn must not install WebView2 on Server Core. |
| Interactive session | `crates/idle-scheduler/src/windows_state.rs:106-110` requires `WTSGetActiveConsoleSessionId` and `WTSQueryUserToken`; it returns `no active console session` without a logged-on console. `services/maintenance-service/src/main.rs:271-278` already treats scheduler start failure as non-fatal. | No interactive-session requirement for service/CLI. Autonomous idle scheduling is unavailable when no console session exists and must remain an honest warning, not a failed install. |
| Windows Update / WSUS | `crates/windows-update/src/windows_impl.rs:39-47,76-87` uses WUA COM and `SetOnline`; WUA retains the machine's configured source. `crates/windows-update/src/execution_windows.rs:59-78` checks WUA installer busy/reboot state before mutation. | Supported on Server, but results are policy-controlled under WSUS. Report WUA discovery/update as degraded with the WSUS-policy note; preserve typed HRESULT/offline/busy/reboot outcomes. |
| PnP / driver servicing | `crates/windows-pnp/src/windows_impl.rs:47-76` uses SetupAPI/Configuration Manager for present devices; `crates/driver-backup/src/windows_impl.rs:20-49` invokes the fixed `%SystemRoot%\\System32\\pnputil.exe` path. | Supported where the Server image exposes the native PnP store and policy permits it. No client-only API is used; runtime qualification must exercise discovery, backup and a consented install. |
| Restore points | `crates/restore-point/src/lib.rs:8-15` documents desktop-only System Restore; `crates/system-repair/src/windows_impl.rs:420-426` deliberately leaves readiness unknown until runtime evidence. | Refused/unavailable on Server SKUs. Driver mutation must fail closed when no fresh restore point can be verified. |
| Power / thermal | `crates/performance-telemetry/src/windows_impl.rs:187-190` samples processor power/thermal limits through `CallNtPowerInformation`; the performance rule consumes those samples in `crates/performance-bottleneck/src/lib.rs:406-451`. | The provider remains read-only, but the product does not offer the client thermal/power optimization surface on Server; capability is unavailable rather than simulated. |
| Game mode | `crates/performance-optimization/src/lib.rs:71-73,222-223` models a Game Mode registry mutation, but no Server-specific provider exists. | Refused on Server; no Game Mode registry write is attempted. |
| Other crates | `crates/system-repair/src/windows_impl.rs:42-109` uses DISM/SFC and WUA preflight; `crates/startup-manager/src/windows_impl.rs` reads existing Task Scheduler state; `crates/windows-foundation/src/lib.rs:77-92` reads documented HKLM metadata; none requires a client SKU. | Keep these paths available and preserve their existing typed failure handling. |

## Server product shape

* **Supported:** Windows Server 2025 or newer, x64/ARM64 builds that meet the
  package architecture, service session-0 operation, `aetherctl`, read-only
  diagnostics, DISM/SFC, PnP inventory/backup, local intelligence, fleet and
  consented service operations. Windows Update remains available but is marked
  policy-dependent when WSUS controls the source.
* **Unavailable (honest typed status):** thermal/power clamp and Game Mode on
  all Server SKUs; System Restore/restore points on all Server SKUs; the desktop
  feature and Start Menu shortcut on Server Core; autonomous idle scheduling in
  a zero-console-session Server Core deployment.
* **Refused:** Server builds below 26100, non-64-bit packages, and any driver
  mutation that cannot prove a fresh restore point and WUA/PnP preflight. No
  capability is simulated and no security boundary is weakened.

The capability matrix will expose `windowsServer` / `windowsServerCore` as the
platform label and use the existing Native/Degraded/NotAvailable envelope. A
missing or unrecognised SKU is conservative: Server-sensitive capabilities are
not reported as native.

## Qualification checklist

1. Build the MSI and Burn bundle with WiX 6.0.2; inspect the MSI
   `LaunchCondition` table and confirm zero ICE findings.
2. On a disposable Windows Server 2025 Evaluation VM, run the MSI on Desktop
   Experience and Server Core. Confirm Server Core installs service/CLI only,
   has no desktop shortcut, and does not chain WebView2.
3. Verify service account/session (`LocalSystem`, session 0), pipe DACL,
   Service SID, install-directory ACLs, repair, and uninstall. Run every CLI
   verb from a non-interactive session.
4. Configure a WSUS policy (or an intentionally offline WUA state) and record
   the exact typed discovery/update outcome; do not call policy failure a
   product failure without the HRESULT.
5. Exercise SetupAPI/PnP inventory and a synthetic-safe driver backup. Only
   perform a real driver mutation with a disposable driver and an observed
   restore-point/preflight result; Server's normal no-System-Restore outcome
   must refuse the mutation.
6. Confirm capability output for Desktop Experience and Server Core, including
   the unavailable thermal/power, Game Mode and restore-point rows.
7. A Parallels Windows Server Evaluation VM is sufficient for SKU, feature,
   service, WSUS, PnP and headless CLI qualification if the chosen Server
   architecture boots there. It is not evidence for physical x86_64 hardware,
   production signing, long-run soak, or a real second fleet target.

## Qualification boundary

No Windows Server SKU has been executed here. All Server statements in this
document are source-derived targets. The Server VM run, including the exact
Parallels architecture choice, remains open.
