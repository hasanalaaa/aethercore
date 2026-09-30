//! P84-02B: the driver search runs as an abortable WUA job under a deadline. The platform code is
//! Windows-only COM, so this reads its source: a search put back as a plain blocking `Search`
//! would hold the drivers page on "Searching" for as long as the agent takes.

#[test]
fn the_driver_search_is_an_abortable_job_under_a_deadline() {
    let source: String = include_str!("../src/windows_impl.rs")
        .split_whitespace()
        .collect();
    let start = source
        .find("pubfndiscover_driver_offers(")
        .expect("discover_driver_offers");
    let body = &source[start..];
    let body = &body[..body[1..].find("pubfn").map_or(body.len(), |end| end + 1)];
    for needle in [
        "search_bounded(",
        "BeginSearch(",
        "RequestAbort()",
        "EndSearch(",
    ] {
        assert!(
            body.contains(needle),
            "the driver search does not call {needle}"
        );
    }
    assert!(
        !body.contains(".Search(&"),
        "the driver search still blocks on a plain Search"
    );
}
