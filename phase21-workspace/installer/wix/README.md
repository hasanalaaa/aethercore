# AetherCore WiX installer

Phase 8 pins WiX Toolset **6.0.2** through `.config/dotnet-tools.json`. WiX v6 is still inside its published consumer security-fix window on the Phase 8 implementation date; upgrades of the pinned tool are deliberate release-engineering changes, never floating CI inputs.

`Product.wxs` builds a per-machine x64 MSI containing only the four signed AetherCore executables. Windows Installer creates/removes the service; the fixed-purpose install hardener runs deferred as LocalSystem after `InstallServices` and before `StartServices`, then applies the unrestricted service SID, delayed-auto configuration, exact service DACL, and NTFS ACL policy without accepting user-provided paths, service names, commands, PowerShell, or `cmd.exe`. The package intentionally does not rely on MSI `ServiceConfig` for SID/delayed-start semantics.

`Bundle.wxs` wraps the MSI with Burn and the Microsoft-signed Evergreen WebView2 bootstrapper. Both entry points admit 64-bit Windows 11 clients (build 22621+) and Windows Server 2025+ member servers (build 26100+). Server Core is a service/CLI-only install: the desktop feature is levelled out by the MSI and the WebView2 chain package is skipped. The bootstrapper payload is downloaded during the release build from Microsoft's documented evergreen link and its Authenticode publisher is verified before it may enter the bundle.

The MSI intentionally preserves `%ProgramData%\AetherCore` state/history during uninstall. Product binaries, shortcuts, registry install markers, and the Windows service are removed. Preserving journals avoids turning uninstall into an implicit data-destruction operation.

## Reproducibility boundary

AetherCore uses deterministic Rust linker settings, lockfiles, pinned toolchains, and `SOURCE_DATE_EPOCH` for SBOM generation. WiX v6 itself still has an open upstream reproducible-MSI gap around package code and summary timestamps. Therefore release verification reports MSI/Bundle reproducibility separately rather than claiming byte-for-byte reproducibility that the tool does not currently provide.
