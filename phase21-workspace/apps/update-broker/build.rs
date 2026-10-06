fn main() {
    // The target decides, not the build host: these are MSVC linker arguments.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        // Like the consent broker: the update broker installs updates and must request
        // elevation itself, not rely on its launcher's "runas" verb (verify-installer-security).
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTUAC:level='requireAdministrator'");
    }
}
