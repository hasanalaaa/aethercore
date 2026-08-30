#![no_main]
use libfuzzer_sys::fuzz_target;
use aethercore_support_bundle::fuzz_sanitize_text;

fuzz_target!(|data: &[u8]| {
    let noise = String::from_utf8_lossy(data);
    let input = format!(
        "{noise} C:\\Users\\Alice\\AppData\\Local\\x.log alice@example.com S-1-5-21-123-456-789-1001 {noise}"
    );
    let (sanitized, report) = fuzz_sanitize_text(&input);
    assert!(!sanitized.contains("C:\\Users\\Alice"));
    assert!(!sanitized.contains("alice@example.com"));
    assert!(!sanitized.contains("S-1-5-21-123-456-789-1001"));
    assert!(sanitized.contains("%USERPROFILE%"));
    assert!(report.user_path_redactions >= 1);
    assert!(report.email_redactions >= 1);
    assert!(report.account_identifier_redactions >= 1);
});
