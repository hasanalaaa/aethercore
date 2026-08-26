//! Phase 29 (T1) — EXPORT_V1: canonical, hash-chained, optionally signed journal export.
//!
//! Format contract (docs/phase29/ARCHITECTURE.md §EXPORT_V1):
//! - JSON envelope with ordered records drawn from EXISTING persistence accessors only
//!   (maintenance executions, plan/journal events, repair timeline). Read-only.
//! - Every record carries `record_hash = sha256(canonical_json(record))`.
//! - Records are chained: `chain_hash[i] = sha256(prev_chain || record_hash)`,
//!   `prev_chain` starts at the all-zeros genesis. The final chain value seals the file
//!   as `digest`.
//! - Header: {schema, generated_unix_ms, record_count, source_db_fingerprint}.
//! - OPTIONAL Ed25519 signature over the digest hex. Keys are NEVER created implicitly:
//!   if no signing key is configured the envelope states `"signed": false` honestly.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// Schema identifier pinned for the lifetime of this format.
pub const EXPORT_SCHEMA: &str = "aethercore.export.v1";
/// All-zero genesis chaining value.
pub const GENESIS_CHAIN: &str = "0000000000000000000000000000000000000000000000000000000000000000";

// ---------------------------------------------------------------------------
// Wire/envelope types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExportRecord {
    /// Record class: "maintenance_execution" | "plan_event" | "repair_timeline".
    pub kind: String,
    /// Monotonic ordering key inside the class (unix ms or seq).
    pub ordinal: i64,
    /// Canonical JSON of the source record (sorted keys via serde_json preserve_order off).
    pub payload: serde_json::Value,
    /// sha256(canonical_json(self minus record_hash/chain_hash)).
    pub record_hash: String,
    /// sha256(prev_chain_hash || record_hash).
    pub chain_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExportHeader {
    pub schema: String,
    pub generated_unix_ms: i64,
    pub record_count: u64,
    /// Fingerprint of the source database (sha256 of its content fingerprint inputs).
    pub source_db_fingerprint: String,
    /// Phase 31 (W9): OPTIONAL correlation id (uuid-v7-format string) threading this
    /// export to the care step / CLI invocation that produced it. Clamped typed:
    /// >128 chars or non-[a-zA-Z0-9-] → dropped at construction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}
/// Phase 31 (W9): typed CorrelationId — uuid-v7-format string, clamped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CorrelationId(String);

impl CorrelationId {
    /// Accepts only sane ids: 1..=128 chars of [0-9a-zA-Z-]. Anything hostile is
    /// dropped (None) rather than truncated into ambiguity.
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty()
            || trimmed.len() > 128
            || !trimmed
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return None;
        }
        Some(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExportSignature {
    /// Ed25519 public key, lowercase hex.
    pub public_key_hex: String,
    /// Ed25519 signature over the ASCII digest hex string.
    pub signature_hex: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExportEnvelope {
    pub header: ExportHeader,
    /// Ascending order by (ordinal, kind) — the canonical replay order.
    pub records: Vec<ExportRecord>,
    /// Final chain value; equals the last record's chain_hash when records exist,
    /// otherwise the genesis constant.
    pub digest: String,
    /// Honest presence flag: true ONLY when an owner-configured key signed the digest.
    pub signed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ExportSignature>,
}

// ---------------------------------------------------------------------------
// Typed verification failures (anti-snake-oil: every rejection names its cause)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum VerifyError {
    /// Envelope is not decodable JSON of the expected shape.
    MalformedEnvelope(String),
    /// header.schema != EXPORT_SCHEMA.
    UnknownSchema(String),
    /// header.record_count disagrees with records.len().
    RecordCountMismatch { declared: u64, actual: usize },
    /// A record's recomputed hash differs from its stored record_hash (tamper point).
    RecordHashMismatch { index: usize },
    /// The running chain diverges from the stored chain_hash (reorder/splice point).
    ChainBreak {
        index: usize,
        expected_chain: String,
    },
    /// Final digest does not equal the last chain value.
    DigestMismatch { expected: String },
    /// "signed": false but a signature object is present (or vice versa).
    SignatureFlagInconsistent,
    /// Signature present but cryptographically invalid over the digest.
    BadSignature,
}

impl core::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MalformedEnvelope(e) => write!(f, "malformed envelope: {e}"),
            Self::UnknownSchema(s) => write!(f, "unknown schema: {s}"),
            Self::RecordCountMismatch { declared, actual } => {
                write!(
                    f,
                    "record_count mismatch: declared {declared}, actual {actual}"
                )
            }
            Self::RecordHashMismatch { index } => {
                write!(f, "record_hash mismatch at record {index}")
            }
            Self::ChainBreak {
                index,
                expected_chain,
            } => {
                write!(
                    f,
                    "chain break at record {index}: expected chain {expected_chain}"
                )
            }
            Self::DigestMismatch { expected } => {
                write!(f, "digest mismatch: file digest must be {expected}")
            }
            Self::SignatureFlagInconsistent => {
                write!(f, "\"signed\" flag inconsistent with signature presence")
            }
            Self::BadSignature => write!(f, "signature invalid over digest"),
        }
    }
}

impl std::error::Error for VerifyError {}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

fn canonical(value: &serde_json::Value) -> String {
    // serde_json with sorted keys — BTreeMap backing for objects keeps output stable.
    serde_json::to_string(value).unwrap_or_default()
}

fn record_hash(kind: &str, ordinal: i64, payload: &serde_json::Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.as_bytes());
    hasher.update(ordinal.to_le_bytes());
    hasher.update(canonical(payload).as_bytes());
    hex_encode(&hasher.finalize())
}

fn chain_step(previous: &str, record_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(previous.as_bytes());
    hasher.update(record_hash.as_bytes());
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Builds a sealed envelope from caller-supplied ordered records and a db fingerprint.
/// Ordering is enforced here (ascending ordinal, stable kind tiebreak).
pub fn build_envelope(
    mut records: Vec<(String, i64, serde_json::Value)>,
    generated_unix_ms: i64,
    source_db_fingerprint: String,
) -> ExportEnvelope {
    build_envelope_with_correlation(records, generated_unix_ms, source_db_fingerprint, None)
}

/// As [`build_envelope`], attaching an optional typed correlation id.
pub fn build_envelope_with_correlation(
    records: Vec<(String, i64, serde_json::Value)>,
    generated_unix_ms: i64,
    source_db_fingerprint: String,
    correlation_id: Option<CorrelationId>,
) -> ExportEnvelope {
    let mut records = records;
    records.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    let mut previous = GENESIS_CHAIN.to_string();
    let mut chained = Vec::with_capacity(records.len());
    for (kind, ordinal, payload) in &records {
        let rh = record_hash(kind, *ordinal, payload);
        let ch = chain_step(&previous, &rh);
        chained.push(ExportRecord {
            kind: kind.clone(),
            ordinal: *ordinal,
            payload: payload.clone(),
            record_hash: rh,
            chain_hash: ch.clone(),
        });
        previous = ch;
    }
    let digest = if chained.is_empty() {
        GENESIS_CHAIN.to_string()
    } else {
        previous
    };
    ExportEnvelope {
        header: ExportHeader {
            schema: EXPORT_SCHEMA.to_string(),
            generated_unix_ms,
            record_count: chained.len() as u64,
            source_db_fingerprint,
            correlation_id: correlation_id.map(|c| c.0),
        },
        digest,
        records: chained,
        signed: false,
        signature: None,
    }
}

/// Signs the envelope's digest in place with an owner-provided Ed25519 keypair.
/// Keys come from disk (`aetherctl keys generate`) — this function never generates.
pub fn sign_envelope(envelope: &mut ExportEnvelope, signing_key: &ed25519_dalek::SigningKey) {
    use ed25519_dalek::Signer as _;
    let verifying = signing_key.verifying_key();
    let signature = signing_key.sign(envelope.digest.as_bytes());
    envelope.signed = true;
    envelope.signature = Some(ExportSignature {
        public_key_hex: hex_encode(&verifying.to_bytes()),
        signature_hex: hex_encode(&signature.to_bytes()),
    });
}

// ---------------------------------------------------------------------------
// Offline verification (zero service dependency)
// ---------------------------------------------------------------------------

/// Full offline verification: recomputes every hash and the chain from scratch.
/// Trusts nothing it cannot recompute.
pub fn verify_envelope(envelope: &ExportEnvelope) -> Result<(), VerifyError> {
    if envelope.header.schema != EXPORT_SCHEMA {
        return Err(VerifyError::UnknownSchema(envelope.header.schema.clone()));
    }
    if envelope.header.record_count != envelope.records.len() as u64 {
        return Err(VerifyError::RecordCountMismatch {
            declared: envelope.header.record_count,
            actual: envelope.records.len(),
        });
    }
    if envelope.signed != envelope.signature.is_some() {
        return Err(VerifyError::SignatureFlagInconsistent);
    }
    let mut previous = GENESIS_CHAIN.to_string();
    for (index, record) in envelope.records.iter().enumerate() {
        let expect_rh = record_hash(&record.kind, record.ordinal, &record.payload);
        if record.record_hash != expect_rh {
            return Err(VerifyError::RecordHashMismatch { index });
        }
        let expect_ch = chain_step(&previous, &expect_rh);
        if record.chain_hash != expect_ch {
            return Err(VerifyError::ChainBreak {
                index,
                expected_chain: expect_ch,
            });
        }
        previous = record.chain_hash.clone();
    }
    let expect_digest = if envelope.records.is_empty() {
        GENESIS_CHAIN.to_string()
    } else {
        previous
    };
    if envelope.digest != expect_digest {
        return Err(VerifyError::DigestMismatch {
            expected: expect_digest,
        });
    }
    if let Some(signature) = &envelope.signature {
        use ed25519_dalek::{Signature, VerifyingKey};
        let key_bytes = decode_hex_fixed::<32>(&signature.public_key_hex)
            .map_err(|_| VerifyError::BadSignature)?;
        let sig_bytes = decode_hex_fixed::<64>(&signature.signature_hex)
            .map_err(|_| VerifyError::BadSignature)?;
        let Ok(key) = VerifyingKey::from_bytes(&key_bytes) else {
            return Err(VerifyError::BadSignature);
        };
        let Ok(sig) = Signature::from_slice(&sig_bytes) else {
            return Err(VerifyError::BadSignature);
        };
        use ed25519_dalek::Verifier as _;
        if key.verify(envelope.digest.as_bytes(), &sig).is_err() {
            return Err(VerifyError::BadSignature);
        }
    }
    Ok(())
}

/// Builds a signing key from an explicit 32-byte seed (owner-provided key file).
/// No key material is created anywhere else in the codebase.
pub fn signing_key_from_seed(seed: &[u8; 32]) -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(seed)
}

/// Decodes a hex string into a fixed-size byte array (length checked).
fn decode_hex_fixed<const N: usize>(input: &str) -> Result<[u8; N], ()> {
    if input.len() != N * 2 {
        return Err(());
    }
    let mut out = [0u8; N];
    for (i, chunk) in input.as_bytes().chunks(2).enumerate() {
        let hi = (chunk[0] as char).to_digit(16).ok_or(())? as u8;
        let lo = (chunk[1] as char).to_digit(16).ok_or(())? as u8;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

/// Parses an envelope from raw file bytes (typed failure on any decode problem).
pub fn parse_envelope_bytes(raw: &[u8]) -> Result<ExportEnvelope, VerifyError> {
    serde_json::from_slice(raw).map_err(|e| VerifyError::MalformedEnvelope(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_records() -> Vec<(String, i64, serde_json::Value)> {
        vec![
            (
                "plan_event".into(),
                100,
                serde_json::json!({"kind": "state_transition", "detail": "Draft→Scanning"}),
            ),
            (
                "maintenance_execution".into(),
                200,
                serde_json::json!({"domain": "cleanup", "outcome": "Succeeded"}),
            ),
            (
                "repair_timeline".into(),
                300,
                serde_json::json!({"action": "verify", "result": "pass"}),
            ),
        ]
    }

    #[test]
    fn golden_path_round_trip_and_signature() {
        let mut env = build_envelope(sample_records(), 1_700_000_000_000, "fp-01".into());
        assert_eq!(env.records.len(), 3);
        assert!(env.verify_ok());
        assert!(!env.signed, "unsigned honesty before any key configured");

        // Owner action: explicit key material (test-only here; CLI uses keys generate).
        let signing = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        sign_envelope(&mut env, &signing);
        assert!(env.signed && env.signature.is_some());
        assert_eq!(verify_envelope(&env), Ok(()));

        // Serialization stability: same content → same digest chain.
        let text = serde_json::to_string(&env).unwrap();
        let reparsed = parse_envelope_bytes(text.as_bytes()).unwrap();
        assert_eq!(reparsed, env);
        assert_eq!(verify_envelope(&reparsed), Ok(()));
    }

    #[test]
    fn tamper_first_record_is_named() {
        let mut env = build_envelope(sample_records(), 0, "fp".into());
        env.records[0].payload["injected"] = serde_json::json!(true);
        assert_eq!(
            verify_envelope(&env),
            Err(VerifyError::RecordHashMismatch { index: 0 })
        );
    }

    #[test]
    fn tamper_middle_record_is_named() {
        let mut env = build_envelope(sample_records(), 0, "fp".into());
        let n = env.records.len();
        env.records[n / 2].ordinal += 1; // flip one field mid-file
        assert!(matches!(
            verify_envelope(&env),
            Err(VerifyError::RecordHashMismatch { .. })
        ));
    }

    #[test]
    fn tamper_last_record_is_named() {
        let mut env = build_envelope(sample_records(), 0, "fp".into());
        let last = env.records.len() - 1;
        env.records[last].record_hash = "0".repeat(64);
        assert_eq!(
            verify_envelope(&env),
            Err(VerifyError::RecordHashMismatch { index: last })
        );
    }

    #[test]
    fn reordered_records_break_chain_typed() {
        let mut env = build_envelope(sample_records(), 0, "fp".into());
        // Records carry (kind, ordinal, payload, record_hash, chain_hash) together, so
        // each record is self-consistent after a swap — but chain_hash embeds its
        // ORIGINAL predecessor, so the running chain diverges at the first swapped
        // position. Typed rejection names that index.
        env.records.swap(0, 2);
        assert!(matches!(
            verify_envelope(&env),
            Err(VerifyError::ChainBreak { index: 0, .. })
        ));
    }

    #[test]
    fn truncation_is_record_count_or_digest_failure() {
        let mut env = build_envelope(sample_records(), 0, "fp".into());
        env.records.pop();
        assert!(matches!(
            verify_envelope(&env),
            Err(VerifyError::RecordCountMismatch { .. }) | Err(VerifyError::DigestMismatch { .. })
        ));
    }

    #[test]
    fn empty_range_seals_at_genesis_and_verifies() {
        let env = build_envelope(Vec::new(), 5, "empty-fp".into());
        assert_eq!(env.digest, GENESIS_CHAIN);
        assert_eq!(env.header.record_count, 0);
        assert_eq!(verify_envelope(&env), Ok(()));
    }

    #[test]
    fn hostile_header_values_rejected_typed() {
        let mut env = build_envelope(sample_records(), 0, "fp".into());
        env.header.schema = "aethercore.export.v9".into();
        assert_eq!(
            verify_envelope(&env),
            Err(VerifyError::UnknownSchema("aethercore.export.v9".into()))
        );
        let mut env = build_envelope(sample_records(), 0, "fp".into());
        env.header.record_count = 999;
        assert!(matches!(
            verify_envelope(&env),
            Err(VerifyError::RecordCountMismatch { .. })
        ));
    }

    #[test]
    fn correlation_ids_are_clamped_typed() {
        use super::CorrelationId;
        // uuid-v7-format string accepted.
        let ok = CorrelationId::parse("018f3f1a-7b9c-7cc3-9c4a-5d6e7f8091a2").unwrap();
        assert_eq!(ok.as_str(), "018f3f1a-7b9c-7cc3-9c4a-5d6e7f8091a2");
        // Hostile inputs dropped typed (None), never truncated.
        assert!(CorrelationId::parse("").is_none());
        assert!(CorrelationId::parse(&"x".repeat(129)).is_none());
        assert!(CorrelationId::parse("bad;drop table users").is_none());
        // Envelope carries it optionally; digest unaffected by presence flag alone.
        let env = build_envelope_with_correlation(sample_records(), 0, "fp".into(), Some(ok));
        assert_eq!(
            env.header.correlation_id.as_deref(),
            Some("018f3f1a-7b9c-7cc3-9c4a-5d6e7f8091a2")
        );
        assert_eq!(verify_envelope(&env), Ok(()));
    }

    #[test]
    fn signed_flag_lies_are_caught() {
        let mut env = build_envelope(sample_records(), 0, "fp".into());
        env.signed = true; // claims signature without one
        assert_eq!(
            verify_envelope(&env),
            Err(VerifyError::SignatureFlagInconsistent)
        );
    }

    impl ExportEnvelope {
        fn verify_ok(&self) -> bool {
            verify_envelope(self).is_ok()
        }
    }
}
