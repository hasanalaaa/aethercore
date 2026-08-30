#![no_main]
use libfuzzer_sys::fuzz_target;
use aethercore_windows_pnp::parse_multi_sz_utf16;

fuzz_target!(|data: &[u8]| {
    let mut units = Vec::with_capacity((data.len() + 1) / 2);
    for chunk in data.chunks(2) {
        units.push(u16::from_le_bytes([chunk[0], *chunk.get(1).unwrap_or(&0)]));
    }
    let parsed = parse_multi_sz_utf16(&units);
    assert!(parsed.iter().all(|value| !value.contains('\0')));
    assert!(parsed.len() <= units.len().saturating_add(1));
});
