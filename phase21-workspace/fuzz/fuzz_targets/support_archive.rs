#![no_main]
use libfuzzer_sys::fuzz_target;
use aethercore_support_bundle::verify_archive;

fuzz_target!(|data: &[u8]| {
    // A random archive should almost always reject. Any accepted archive must be deterministically
    // accepted on a second verification with the same independent fingerprint.
    let fingerprint = "00".repeat(32);
    let first = verify_archive(data, &fingerprint);
    if first.is_ok() {
        assert!(verify_archive(data, &fingerprint).is_ok());
    }
});
