fn main() {
    // The desktop is the unprivileged half and must say so: an explicit asInvoker request
    // (verify-installer-security, PHASE_8). Tauri's default manifest carries only the
    // Common-Controls v6 dependency, which the dialogs still need and app.manifest keeps.
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("tauri build script failed");
}
