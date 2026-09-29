//! P78-01: the Windows health probe answers from WUA's local cache. An assessment the user starts
//! to look at their machine must not search Microsoft Update; the online path belongs to driver
//! discovery, behind an explicit scope. The probe is Windows-only COM code, so this reads its source.

#[test]
fn the_health_probe_never_searches_online() {
    let source = include_str!("../src/windows_impl.rs");
    let start = source
        .find("pub fn probe_update_health")
        .expect("probe_update_health is in windows_impl.rs");
    let end = source[start..]
        .find("fn update_probe_error")
        .expect("update_probe_error follows the probe")
        + start;
    let probe = &source[start..end];
    for online in ["VARIANT_BOOL(-1)", "VARIANT_TRUE", "SearchScope::Online"] {
        assert!(
            !probe.contains(online),
            "the health probe asks for an online search: {online}"
        );
    }
    assert!(
        probe.contains("SetOnline(VARIANT_FALSE)"),
        "the health probe must say explicitly that its search is local"
    );
}
