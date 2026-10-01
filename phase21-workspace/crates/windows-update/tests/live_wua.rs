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

#[test]
#[ignore = "reads the local Windows Update Operational event channel without changing it"]
fn reads_live_client_errors_without_an_online_search() {
    let events = aethercore_windows_update::query_client_errors(200)
        .expect("the local client channel should be readable");
    assert!(events.errors.len() + events.unknown_events as usize <= 200);
    for event in events.errors {
        assert_eq!(event.event_id, 25);
    }
}
