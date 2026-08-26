//! Phase 29/30 fuzz targets — lib facade for cargo-fuzz.
//!
//! `cargo fuzz run mysql_slow_log_parse -- -runs=200` etc. Each target wraps one
//! untrusted-input parser with the hard capacity bounds from production.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = aethercore_db_diagnostics::parse_mysql_slow_log(data);
});
