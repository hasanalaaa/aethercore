#![cfg(windows)]

#[test]
#[ignore = "performs a live Windows Update Agent applicability scan"]
fn discovers_live_driver_offers_without_installing() {
    let result = aethercore_windows_update::discover_driver_offers(
        aethercore_windows_update::SearchScope::Online,
    )
    .expect("WUA driver discovery should complete on a configured Windows client");

    for offer in result.offers {
        assert!(!offer.update_id.trim().is_empty());
        assert!(offer.revision >= 0);
        assert!(offer.max_download_bytes >= offer.min_download_bytes);
    }
}
