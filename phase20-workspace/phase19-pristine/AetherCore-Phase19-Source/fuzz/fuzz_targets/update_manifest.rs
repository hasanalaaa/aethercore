#![no_main]
use libfuzzer_sys::fuzz_target;
use aethercore_update_engine::{
    MANIFEST_SCHEMA, fuzz_parse_and_validate_manifest_bytes, parse_version, safe_id,
    validate_https_url,
};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let _ = parse_version(&text);
    let _ = safe_id(&text);
    let _ = validate_https_url(&text);

    if let Ok(manifest) = fuzz_parse_and_validate_manifest_bytes(data, "stable", 1_800_000_000_000) {
        assert_eq!(manifest.schema, MANIFEST_SCHEMA);
        assert_eq!(manifest.channel, "stable");
        assert!(manifest.sequence > 0);
        assert!(manifest.expires_unix_ms > manifest.generated_unix_ms);
        for release in manifest.releases {
            assert!(safe_id(&release.release_id));
            assert!(parse_version(&release.version).is_some());
            assert!(validate_https_url(&release.package.url).is_ok());
            assert_eq!(release.package.sha256.len(), 64);
        }
    }
});
