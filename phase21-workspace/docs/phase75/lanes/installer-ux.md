# P75 lane `installer-ux` (Wave 2) — evidence (ledger `DBT-P55-009`, `DBT-P55-008`, `DBT-P75-046`)

Branch `lane/installer-ux`, from `main` `174a9e4`. No dependency or lockfile change.

Scope used: `installer/wix/Bundle.wxs`, new `installer/wix/theme/**`, `installer/wix/Product.wxs`
(launch conditions), `scripts/build-installer.ps1` (`-d ThemeDir`), `.github/workflows/windows-installer.yml`
(focus gate), and one gate in its own commit: `scripts/static_validate.py`
`phase8_msi_upgrade_and_os_gate`.

Windows facts were measured first on a throwaway push-triggered probe (`probe/p75-installer-ux`,
~3 min on `windows-2025`): a tiny MSI chained by two bundles, one with WiX's stock
`hyperlinkLicense` theme and one with this lane's theme, driven by UI Automation.

## Rows

| row | was | now | measured |
|---|---|---|---|
| `DBT-P55-009` | `setup.exe /uninstall` (the Add/Remove Programs command) shows the Modify chooser with **Repair** focused: the first visible tab stop of WiX's stock theme | the bundle ships its own theme (WiX v6.0.2 `HyperlinkTheme.xml` + `.wxl`, MS-RL headers kept) with `UninstallButton` declared and placed before `RepairButton`; Repair stays available (`phase8_msi_repair_not_disabled`) | probe `36238635638`: stock `focused='Repair'` (buttons Repair x=516, Uninstall x=596); ours `focused='Uninstall'` (Uninstall x=516, Repair x=596). Gate added to `bundle-log-acl-probe` on the real bundle |
| `DBT-P55-008` | one English `thm.wxl`, no language subdirectory, so every probe fell back to English (P71) | `1025\thm.wxl` (all 64 string ids, Arabic) and `1025\thm.xml` (layout mirrored by coordinates, text right-aligned with `SS_RIGHT`/`LWS_RIGHT`/`BS_RIGHT\|BS_LEFTTEXT`) as bundle payloads. Burn probes `<lcid>\` first, for the UI language or `/lang 1025` | probe `36238635638`, `/uninstall /lang 1025`: caption `إعداد P75Uxours`, `focused='إزالة التثبيت'`, logo x=687 (right), Cancel x=288 (left); screenshot in the run's `ux-shots` artifact. The first Arabic attempt (`36238533197`) exited 0x57 before any window: `Failed to load image from file: ...\.ba\1025\logo.png` — thmutil resolves `ImageFile` against the theme's own directory; fixed with `..\logo.png` (`07efb7b`) |
| `DBT-P75-046` (new) | none of the MSI's three launch conditions carried `Installed OR`, and Windows Installer evaluates them on uninstall and repair too: a machine that stopped meeting one after install (OS change, a VC++ runtime another uninstaller removed) could no longer remove or repair AetherCore | every condition is `Installed OR (<condition>)`; first installs are gated exactly as before; `phase8_msi_upgrade_and_os_gate` now requires the form on every `<Launch>` | probe `36238635638`, same package with the launch condition forced false at uninstall: without `Installed OR` → `msiexec /x` exit **1603**, log `P75 probe requirement missing`; with it → exit **0**, `Removal completed successfully` |

Also: the stock `HelpText` said the log goes to `%TEMP%`; `Bundle.wxs` puts it in `%WINDIR%\Temp`
(`DBT-P49-003`), and both languages now say so.

## Gate change (own commit)

`phase8_msi_upgrade_and_os_gate` pinned the OS condition's exact text, so a launch condition that
blocked uninstall passed. It now pins the `Installed OR (...)` form and requires every `<Launch>`
in `Product.wxs` to start with `Installed OR `. Planted regression (`Condition="VCRUNTIMEVERSION"`)
→ `"failed": ["phase8_msi_upgrade_and_os_gate"]`; restored → `"failed": []`.

## Not done, and why

* The title bar stays left-to-right: thmutil has no way to set `WS_EX_LAYOUTRTL` on its window
  (`thmutil.cpp` reads `HexStyle`, a `WS_*` style, for the window; extended styles only for
  ListView). Text inside the window is mirrored and right-aligned.
* The UI-language path (`GetUserDefaultUILanguage()` = 1025) is not measured: it needs an
  Arabic language pack and a new logon on the runner. `/lang 1025` reaches the same
  `LocProbeForFile` lookup with the language passed explicitly, which is what was measured.
* The MSI's own strings (launch-condition messages, shown only to a direct `msiexec`) stay English:
  localizing an MSI means one package or transform per culture, a release-shape change.

## Local proof (macOS)

`static_validate.py` → `"failed": []`; `test_gate_readers.py` → all 14 fail closed;
`ps_marker_scan.py` → assertions=234 failed=0 unmeasured=3; `enterprise-adversarial-audit.py`,
`phase15-security-audit.py`, `phase35-gd-proofs.py`, `test_windows_server_support.py` → pass;
`phase35-adversarial-audit.py` fails identically on `origin/main` (three missing Phase 35
contracts), not introduced here. No Rust changed. The compile verdict for `Bundle.wxs` and the
themes is CI's `build-release.ps1` and the `windows-installer.yml` run cited in the PR.
