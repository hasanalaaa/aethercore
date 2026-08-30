fn main() {
    #[cfg(windows)]
    {
        // The broker is deliberately the only interactive executable that requires elevation.
        // MSVC's linker embeds this UAC execution level in the PE manifest; uiAccess defaults to false.
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTUAC:level='requireAdministrator'");
    }
}
