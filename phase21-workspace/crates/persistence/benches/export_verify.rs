//! Phase 31 (W8) — EXPORT_V1 verify baseline over 10k records.

use aethercore_persistence::export;
use criterion::{Criterion, criterion_group, criterion_main};

fn envelope_10k() -> export::ExportEnvelope {
    let records: Vec<(String, i64, serde_json::Value)> = (0..10_000)
        .map(|i| {
            (
                "plan_event".to_string(),
                i as i64,
                serde_json::json!({ "seq": i, "detail": format!("row-{i}") }),
            )
        })
        .collect();
    export::build_envelope(records, 1_700_000_000_000, "bench-fp".into())
}

fn bench_export_verify_10k(c: &mut Criterion) {
    let env = envelope_10k();
    let bytes = serde_json::to_vec(&env).unwrap();
    c.bench_function("export_verify_10k_records", |b| {
        b.iter(|| {
            let parsed = export::parse_envelope_bytes(&bytes).expect("parse");
            export::verify_envelope(&parsed).expect("verify")
        })
    });
}

criterion_group!(benches, bench_export_verify_10k);
criterion_main!(benches);
