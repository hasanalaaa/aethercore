fn main() {
    #[cfg(windows)]
    {
        // Like the consent broker: the update broker installs updates and must request
        // elevation itself, not rely on its launcher's "runas" verb (verify-installer-security).
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTUAC:level='requireAdministrator'");
    }
}
