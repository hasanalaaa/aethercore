# Windows Installer

The canonical definition is WiX v4 `installer/wix/Product.wxs` and the existing Tauri bundling path. The stable UpgradeCode and explicit component GUIDs cover the desktop, consent/update brokers, maintenance service, trust configuration and install hardener. Program Files contains signed binaries/trust; ProgramData contains state/logs and is preserved across upgrade/repair.

Fresh install validates the package, creates owned directories, registers the LocalSystem maintenance service, applies fixed ACL/Service SID policy and starts it. Upgrade stops safely, preserves state and uses MSI major-upgrade rollback where available. Repair restores signed binaries/assets without resetting configuration. Uninstall stops/unregisters the service and removes only AetherCore-owned binaries/transient files; fleet trust and user data remain unless an explicit data-removal mode is added.

This macOS host proves definition composition, GUID/version rules and exclusion policy only. MSI runtime, UAC, ACLs, service registration, reboot recovery and Authenticode are Phase 36 qualification items.
