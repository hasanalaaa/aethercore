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

/// P84-02B: the abortable search against the real agent, reading its local cache only (no
/// network), and the day Windows last searched online. A read-only probe for the PC.
#[test]
#[ignore = "reads the live Windows Update Agent's local cache"]
fn searches_the_local_cache_as_an_abortable_job() {
    let started = std::time::Instant::now();
    let result = aethercore_windows_update::discover_driver_offers(
        aethercore_windows_update::SearchScope::LocalCacheOnly,
    )
    .expect("a local-cache driver search completes");
    eprintln!(
        "local-cache search: {} offer(s), {} warning(s), {} ms; Windows last searched online: {:?}",
        result.offers.len(),
        result.warnings.len(),
        started.elapsed().as_millis(),
        aethercore_windows_update::last_online_search_iso()
    );
}
