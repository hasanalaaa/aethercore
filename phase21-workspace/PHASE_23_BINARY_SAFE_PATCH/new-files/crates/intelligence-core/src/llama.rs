//! Phase 23 — model artifact integrity (I5, fail-closed) + the LlamaCpp reasoner.
//!
//! The on-device model path is compiled behind the `local-model` feature. When that
//! feature is off (the shipped default until an artifact is committed), the selector
//! runs the deterministic fallback and the renderer's engine badge says so honestly.
//!
//! MODEL SELECTION NOTE (documented per contract): the target artifact class is a
//! ≤2 GB q4_k_m instruct model with strong structured-JSON compliance; candidate:
//! Qwen2.5-3B-Instruct (Apache-2.0) or Llama-3.2-3B-Instruct (Llama 3.2 community
//! license). Reasoning happens in English internally; summaries are template-
//! localized by the renderer catalogs. No binary is fabricated here — activation is
//! manual: build with `--features local-model`, place the GGUF under assets/models/,
//! record its sha256 in models.manifest.json, and set the feature at packaging time.

use sha2::Digest;
use std::path::Path;

/// Pinned artifact manifest entry (I5). The loader refuses any mismatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelManifestEntry {
    pub file_name: String,
    pub sha256_hex: String,
    pub declared_ram_budget_bytes: u64,
}

/// Computes the sha256 of the file at `model_path` and compares it to the pinned
/// entry. ANY mismatch (or unreadable file) fails closed: Err, never "load anyway".
pub fn verify_model_hash(model_path: &Path, pinned: &ModelManifestEntry) -> Result<(), String> {
    if pinned.declared_ram_budget_bytes > crate::engine::MAX_MODEL_RAM_BUDGET_BYTES {
        return Err(format!(
            "declared RAM budget {} exceeds cap",
            pinned.declared_ram_budget_bytes
        ));
    }
    let bytes = std::fs::read(model_path).map_err(|e| format!("model read failed: {e}"))?;
    let digest = sha2::Sha256::digest(&bytes);
    let actual = {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(digest.len() * 2);
        for &b in digest.iter() {
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
        out
    };
    if actual != pinned.sha256_hex.to_lowercase() {
        // Fail-closed: refuse on any byte difference (tamper / corruption).
        return Err("model hash mismatch: refusing to load".into());
    }
    Ok(())
}

/// LlamaCpp-backed reasoner (feature-gated). Without the `local-model` feature this
/// type still exists but its inference always reports unavailability, so the selector
/// degrades to fallback — keeping call sites identical in both builds.
pub struct LlamaCppReasoner {
    loaded: bool,
    #[allow(dead_code)]
    context_chars: usize,
}

impl LlamaCppReasoner {
    /// Bounded context window contract (chars, conservative for small quantized models).
    pub const MAX_PROMPT_CHARS: usize = 8_192;

    pub fn new() -> Self {
        Self {
            loaded: false,
            context_chars: Self::MAX_PROMPT_CHARS,
        }
    }

    fn render_prompt(pack_json: &str, question: &str) -> String {
        // Temperature-0 JSON-only prompt contract (INSIGHT_SCHEMA_V1). Evidence packs
        // are pre-typed structured data; hostile text can only appear inside bounded
        // detail fields, never as instructions (threat-model mitigation).
        format!(
            "You are an offline diagnostic advisor. Answer ONLY with a JSON array of \
             insights matching INSIGHT_SCHEMA_V1: [{{\"summaryKey\":\"...\",\"explanation\":\
             \"...\",\"confidence\":\"Weak|Moderate|Strong\",\"citations\":[{{\"evidenceId\":\
             \"...\",\"surface\":\"bottleneckReport|repairDiagnosis|timelinePattern|\
             maintenanceHistory\"}}]}}]. Every insight MUST cite at least one evidenceId \
             from the pack. Question: {question}\nEvidencePack: {pack_json}"
        )
    }
}

impl Default for LlamaCppReasoner {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::engine::LocalReasoner for LlamaCppReasoner {
    fn load(&mut self, model_path: &Path) -> Result<(), String> {
        #[cfg(feature = "local-model")]
        {
            // Real binding site: llama_cpp_2 load with bounded context. Compiled only
            // with --features local-model; see ARCHITECTURE.md for activation.
            let _ = model_path;
            Err(
                "local-model feature enabled but llama backend wiring lands with the \
                 committed artifact (QD-023-001); fallback remains active"
                    .into(),
            )
        }
        #[cfg(not(feature = "local-model"))]
        {
            let _ = model_path;
            Err("local-model feature not compiled; deterministic fallback is the engine".into())
        }
    }

    fn is_loaded(&self) -> bool {
        self.loaded
    }

    fn infer(
        &self,
        pack: &crate::model::TypedEvidencePack,
        question: &str,
        deadline: std::time::Instant,
    ) -> Result<Vec<crate::model::Insight>, String> {
        let prompt =
            Self::render_prompt(&serde_json::to_string(pack).unwrap_or_default(), question);
        let _ = deadline;
        #[cfg(feature = "local-model")]
        {
            let _ = prompt;
            Err("llama backend pending committed artifact (QD-023-001)".into())
        }
        #[cfg(not(feature = "local-model"))]
        {
            let _ = prompt;
            Err("local-model feature not compiled".into())
        }
    }
}
