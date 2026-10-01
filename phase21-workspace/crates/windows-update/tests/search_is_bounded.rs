//! P84-02B: the driver search runs as an abortable WUA job under a deadline. The platform code is
//! Windows-only COM, so this reads its source: a search put back as a plain blocking `Search`
//! would hold the drivers page on "Searching" for as long as the agent takes.

#[test]
fn the_driver_search_is_an_abortable_job_under_a_deadline() {
    let source: String = include_str!("../src/windows_impl.rs")
        .split_whitespace()
        .collect();
    assert!(
        source.contains("discover_driver_offers_with_keepalive(scope,())"),
        "direct read-only probes must delegate with a unit guard"
    );
    let start = source
        .find("pubfndiscover_driver_offers_with_keepalive<G:Send+'static>")
        .expect("the resource-owning search entrypoint");
    let body = &source[start..];
    let body = &body[..body[1..].find("pubfn").map_or(body.len(), |end| end + 1)];
    for needle in [
        "search_bounded(deadline,keepalive,",
        "BeginSearch(",
        "RequestAbort()",
        "EndSearch(",
    ] {
        assert!(
            body.contains(needle),
            "the driver search does not call {needle}"
        );
    }
    // The one plain Search is the agent's own refusal of an abortable job (0x80070005, measured
    // from a network-logon session); it still runs on the bounded thread.
    let plain: Vec<_> = body.match_indices(".Search(&").map(|(at, _)| at).collect();
    assert_eq!(plain.len(), 1, "the driver search blocks on a plain Search");
    let refusal = body.find("E_ACCESSDENIED=>").expect("the refusal branch");
    assert!(
        refusal < plain[0] && plain[0] - refusal < 200,
        "a plain Search outside the refusal branch"
    );
}
