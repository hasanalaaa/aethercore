//! Versioned machine envelope (`aethercore.aetherctl.v1`).
//!
//! Every JSON output is emitted through [`emit_envelope`], which parse-backs the exact
//! bytes through the strict reader ([`parse_envelope_strict`], unknown fields rejected)
//! before they reach stdout — the envelope can never drift from its own schema.

use serde::{Deserialize, Serialize};

pub const ENVELOPE_SCHEMA: &str = "aethercore.aetherctl.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub schema: String,
    pub command: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<EnvelopeError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeError {
    pub kind: String,
    pub message_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Strict reader: rejects unknown top-level and error-object fields (schema pin).
pub fn parse_envelope_strict(raw: &str) -> Result<Envelope, serde_json::Error> {
    serde_json::from_str::<Envelope>(raw)
}

fn strict_parse(value: serde_json::Value) -> Result<Envelope, serde_json::Error> {
    parse_envelope_strict(&value.to_string())
}

/// Builds + self-verifies a success envelope.
pub fn success(command: &str, data: serde_json::Value) -> Result<Envelope, serde_json::Error> {
    strict_parse(serde_json::json!({
        "schema": ENVELOPE_SCHEMA,
        "command": command,
        "ok": true,
        "data": data,
    }))
}

/// Builds + self-verifies a failure envelope.
pub fn failure(
    command: &str,
    kind: &str,
    message_key: &str,
    detail: Option<String>,
) -> Result<Envelope, serde_json::Error> {
    let err = match detail {
        Some(detail) => serde_json::json!({
            "kind": kind,
            "message_key": message_key,
            "detail": detail,
        }),
        None => serde_json::json!({ "kind": kind, "message_key": message_key }),
    };
    strict_parse(serde_json::json!({
        "schema": ENVELOPE_SCHEMA,
        "command": command,
        "ok": false,
        "error": err,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_envelope_round_trips() {
        let envelope = success("service detect", serde_json::json!({"state": "Offline"})).unwrap();
        assert_eq!(envelope.schema, ENVELOPE_SCHEMA);
        assert!(envelope.ok);
        assert_eq!(envelope.data.unwrap()["state"], "Offline");
        assert!(envelope.error.is_none());
    }

    #[test]
    fn failure_envelope_round_trips() {
        let envelope = failure(
            "care start",
            "ConsentRequired",
            "cli.consent.required",
            None,
        )
        .unwrap();
        assert!(!envelope.ok);
        let error = envelope.error.unwrap();
        assert_eq!(error.kind, "ConsentRequired");
        assert_eq!(error.message_key, "cli.consent.required");
    }

    #[test]
    fn unknown_top_level_field_rejected_on_parse_back() {
        let raw =
            r#"{"schema":"aethercore.aetherctl.v1","command":"x","ok":true,"data":{},"bogus":1}"#;
        assert!(parse_envelope_strict(raw).is_err());
    }

    #[test]
    fn unknown_error_field_rejected_on_parse_back() {
        let raw = r#"{"schema":"aethercore.aetherctl.v1","command":"x","ok":false,"error":{"kind":"k","message_key":"m","extra":true}}"#;
        assert!(parse_envelope_strict(raw).is_err());
    }

    #[test]
    fn missing_schema_rejected() {
        let raw = r#"{"command":"x","ok":true}"#;
        assert!(parse_envelope_strict(raw).is_err());
    }
}
