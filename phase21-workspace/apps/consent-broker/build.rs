fn main() {
    // The target decides, not the build host: these are MSVC linker arguments.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        // The two brokers are the only executables that request elevation (the update broker too).
        // MSVC's linker embeds this UAC execution level in the PE manifest; uiAccess defaults to false.
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTUAC:level='requireAdministrator'");
    }
}
