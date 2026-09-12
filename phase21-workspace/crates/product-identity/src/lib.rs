//! The product's own identity strings, decided once.
//!
//! DBT-P46-D1/D2 (§46.4 Part 0.D): these values had no single decider. The
//! Windows service name was declared independently in three places and the
//! product/folder name appeared as a bare literal 23 times across 9 files in 7
//! crates. Every occurrence agreed on the day they were counted, which is
//! exactly what makes the shape dangerous: nothing would have failed if one of
//! them had drifted, and the IPC peer check and the installer hardener would
//! then have been guarding two different services.
//!
//! `crates/ipc`'s `PIPE_NAME` is the pattern this follows — one `pub const`,
//! consumed everywhere, never re-typed.
//!
//! **These are published contracts (§4).** The service name is registered with
//! the SCM and baked into installed machines' SDDL and log ACLs; the product
//! name is the installation directory, the per-machine data directory, and the
//! update protocol's `product_id`. Changing a value here renames things on
//! disk that already exist on real installs. Deduplicating them is free;
//! editing them is not.

/// The Windows service name, as registered with the Service Control Manager.
pub const SERVICE_NAME: &str = "AetherCoreMaintenance";

/// The service's own virtual account, `NT SERVICE\<service name>`.
///
/// This is what the IPC layer resolves to a SID to authenticate the pipe's
/// owner, and what the installer hardener grants ACLs to. It was previously
/// hardcoded twice in full, rather than derived from the name above — so the
/// two halves of one identity could drift apart independently.
pub fn service_principal() -> String {
    format!(r"NT SERVICE\{SERVICE_NAME}")
}

/// The product name, as it appears in the installation directory, the
/// per-machine data directory, and the update protocol's `product_id`.
pub const PRODUCT_NAME: &str = "AetherCore";

#[cfg(test)]
mod tests {
    use super::*;

    /// The literal values, pinned. This is the one place where writing them out
    /// by hand is the point: if someone edits a constant above, this test is
    /// what tells them they are changing a published contract and not just a
    /// string — installed machines already carry these names on disk.
    #[test]
    fn identity_strings_are_the_ones_already_installed_on_real_machines() {
        assert_eq!(SERVICE_NAME, "AetherCoreMaintenance");
        assert_eq!(PRODUCT_NAME, "AetherCore");
        assert_eq!(service_principal(), r"NT SERVICE\AetherCoreMaintenance");
    }

    /// The principal is derived, not re-typed: the two halves of one identity
    /// cannot drift apart again.
    #[test]
    fn the_principal_is_derived_from_the_service_name() {
        assert!(service_principal().ends_with(SERVICE_NAME));
        assert_eq!(service_principal(), format!(r"NT SERVICE\{}", SERVICE_NAME));
    }
}
