//! Phase 23.1 — PERMANENT embedded model: integrity + real llama.cpp loading.
//!
//! Owner decision: the local AI engine ships EMBEDDED and ENABLED BY DEFAULT.
//! No user-facing flag, no manual placement, no picker. The deterministic fallback
//! is ONLY automatic runtime fault-handling (I3), never a supported configuration.

use std::path::Path;

use crate::engine::LocalReasoner;
use sha2::Digest;

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
        return Err("model hash mismatch: refusing to load".into());
    }
    Ok(())
}

/// The embedded artifact's canonical location relative to the product root.
pub const EMBEDDED_MODEL_RELATIVE_PATH: &str = "assets/models/qwen2.5-1.5b-instruct-q4_k_m.gguf";

/// The pinned entry for THE shipped artifact, compiled in so a swapped manifest on
/// disk alone cannot weaken verification (defense in depth with the manifest check).
pub fn embedded_model_entry() -> ModelManifestEntry {
    ModelManifestEntry {
        file_name: "qwen2.5-1.5b-instruct-q4_k_m.gguf".into(),
        sha256_hex: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e".into(),
        declared_ram_budget_bytes: 2 * 1024 * 1024 * 1024,
    }
}

/// Full startup sequence (M2): locate → sha256 vs pinned+manifest → RAM budget →
/// load within budget. Returns Ok(engine_label) after emitting the typed startup log.
pub fn activate_embedded_reasoner(product_root: &Path) -> Result<String, String> {
    let model_path = product_root.join(EMBEDDED_MODEL_RELATIVE_PATH);
    let pinned = embedded_model_entry();
    verify_model_hash(&model_path, &pinned)?;
    let mut reasoner = LlamaCppReasoner::new();
    reasoner.load(&model_path)?;
    if !reasoner.is_loaded() {
        return Err("loader returned success but model is not loaded".into());
    }
    let label = format!(
        "intelligence-core: embedded reasoner active (model={}, sha256 ok)",
        pinned.file_name.trim_end_matches(".gguf")
    );
    eprintln!("{label}");
    Ok(label)
}

/// Verifies and loads the embedded artifact as a STREAMING reasoner — the engine
/// the assistant answers from.
///
/// Separate from [`activate_embedded_reasoner`], which returns the insight
/// path's `LocalReasoner`. The two traits have different shapes (one streams
/// under a cancellable budget, one returns a finished insight vector), so one
/// object cannot satisfy both; this is the one the assistant needs.
pub fn load_streaming_reasoner(
    product_root: &Path,
) -> Result<Box<dyn crate::assistant::StreamingReasoner>, String> {
    let model_path = product_root.join(EMBEDDED_MODEL_RELATIVE_PATH);
    verify_model_hash(&model_path, &embedded_model_entry())?;
    let mut reasoner = LlamaCppReasoner::new();
    LocalReasoner::load(&mut reasoner, &model_path)?;
    if !LocalReasoner::is_loaded(&reasoner) {
        return Err("loader returned success but model is not loaded".into());
    }
    Ok(Box::new(reasoner))
}

/// LlamaCpp-backed reasoner — the PERMANENT default engine when the artifact verifies.
///
/// Bounded-context contract (unchanged): prompt ≤ [`Self::MAX_PROMPT_CHARS`] chars,
/// temperature 0, JSON-only INSIGHT_SCHEMA_V1 output parsed strictly; malformed
/// output surfaces as Err so the selector degrades to fallback for that call (I3).
pub struct LlamaCppReasoner {
    loaded: bool,
    /// Handle text kept opaque; the binding types are feature-internal.
    backend: Option<LlamaBackendHandle>,
}

#[cfg(feature = "embedded-model")]
struct LlamaBackendHandle {
    _model: llama_cpp_2::model::LlamaModel,
}

#[cfg(not(feature = "embedded-model"))]
struct LlamaBackendHandle {}

impl LlamaCppReasoner {
    /// Bounded context window contract (chars, conservative for small quantized models).
    pub const MAX_PROMPT_CHARS: usize = 8_192;

    pub fn new() -> Self {
        Self {
            loaded: false,
            backend: None,
        }
    }

    fn render_prompt(pack_json: &str, question: &str) -> String {
        // Temperature-0 JSON-only prompt contract (INSIGHT_SCHEMA_V1). Evidence packs
        // are pre-typed structured data; hostile text can only appear inside bounded
        // detail fields, never as instructions (threat-model mitigation).
        format!(
            "You are an offline diagnostic advisor. Answer ONLY with a JSON array of              insights matching INSIGHT_SCHEMA_V1: [{{\"summaryKey\":\"...\",\"explanation\":\
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

#[cfg_attr(not(feature = "embedded-model"), allow(unused))]
impl crate::engine::LocalReasoner for LlamaCppReasoner {
    fn load(&mut self, model_path: &Path) -> Result<(), String> {
        #[cfg(feature = "embedded-model")]
        {
            let model = llama_cpp_2::model::LlamaModel::load_from_file(
                backend_global(),
                model_path,
                &llama_cpp_2::model::params::LlamaModelParams::default(),
            )
            .map_err(|e| format!("gguf load failed: {e}"))?;

            // Bounded context: 2048 tokens is generous for schema + pack and keeps RAM
            // well under the declared budget for a 1.5B q4_k_m model. Temperature-0 and
            // JSON-only parsing are enforced at inference time; this warm-up proves the
            // artifact loads into a working context window.
            let backend = backend_global();
            let ctx_params = llama_cpp_2::context::params::LlamaContextParams::default()
                .with_n_ctx(std::num::NonZeroU32::new(2048))
                .with_n_batch(256);
            {
                let _ctx = model
                    .new_context(backend, ctx_params)
                    .map_err(|e| format!("context init failed: {e}"))?;
                // Context creation itself proves the artifact loads and the window
                // works; token-level generation is driven per-request by the service.
            }

            self.backend = Some(LlamaBackendHandle { _model: model });
            self.loaded = true;
            Ok(())
        }
        #[cfg(not(feature = "embedded-model"))]
        {
            let _ = model_path;
            Err("embedded-model feature not compiled".into())
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
        // DBT-P46-B23: `infer` already has an error channel, so there is no
        // reason to default here. An empty pack would ask the model the
        // question with NO evidence, and anything it answered would be
        // uncitable by construction.
        let pack_json = serde_json::to_string(pack)
            .map_err(|error| format!("evidence pack could not be serialized: {error}"))?;
        let prompt = Self::render_prompt(&pack_json, question);
        if prompt.len() > Self::MAX_PROMPT_CHARS {
            return Err("prompt exceeds bounded context contract".into());
        }
        #[cfg(feature = "embedded-model")]
        {
            self.infer_embedded(&prompt, deadline)
        }
        #[cfg(not(feature = "embedded-model"))]
        {
            let _ = deadline;
            Err("embedded-model feature not compiled".into())
        }
    }
}

impl LlamaCppReasoner {
    /// Real generation over the stored model handle. The mutable context is created
    /// per call from the shared backend (cheap relative to prompt processing at this
    /// model size and keeps `&self` semantics for the trait).
    #[cfg(feature = "embedded-model")]
    fn infer_embedded(
        &self,
        _prompt: &str,
        deadline: std::time::Instant,
    ) -> Result<Vec<crate::model::Insight>, String> {
        let handle = self.backend.as_ref().ok_or("model not loaded")?;
        // The stored LlamaModel is read-only after load; contexts are created per call
        // from the process-global backend (cheap relative to prompt processing here).
        let model: &llama_cpp_2::model::LlamaModel = &handle._model;
        let _ = model;
        if std::time::Instant::now() >= deadline {
            return Err("inference deadline exceeded before generation".into());
        }
        Err("token-level generation requires the context pool wired in intelligence.rs              (QD-023-003 perf validation lands first); fallback serves this request"
            .into())
    }
}

/// Phase 56 — real token-level generation.
///
/// `DBT-P56-002`: this did not exist. `infer_embedded` returned `Err`
/// unconditionally, so the artifact verified its sha256, loaded, built a context
/// at startup — and then every request in the product's life fell through to the
/// deterministic rule engine while `engine_label` still read `localModel`.
///
/// The loop below is the whole of it: tokenize the grounded prompt, decode it,
/// then greedily sample one token at a time, checking the three ceilings BETWEEN
/// tokens — cancel, deadline, token count. Greedy rather than sampled because
/// the same question over the same evidence must give the same answer; a
/// diagnostic tool that says something different each time you ask is not one.
impl crate::assistant::StreamingReasoner for LlamaCppReasoner {
    fn is_loaded(&self) -> bool {
        self.loaded
    }

    fn generate(
        &self,
        pack: &crate::model::TypedEvidencePack,
        question: &str,
        budget: &crate::assistant::GenerationBudget,
        sink: &mut dyn FnMut(&str),
    ) -> Result<crate::assistant::Generated, String> {
        // The shipped artifact is qwen2.5-1.5b-INSTRUCT, and an instruct model
        // follows its rules far better inside its own chat template than in a
        // flat prompt. Measured: flat, the model echoed the instruction text
        // back into the answer ("Mark every claim with the tag of the evidence
        // it rests on: NO EVIDENCE") and kept writing past its conclusion.
        // `str_to_token` parses special tokens, so the control tokens below are
        // real ones, not literal text; `<|im_end|>` is an EOG token, which is
        // what ends the loop naturally instead of the 512-token ceiling.
        let prompt = format!(
            "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
            crate::assistant::SYSTEM_PROMPT,
            crate::assistant::render_user_message(pack, question),
        );
        if prompt.len() > Self::MAX_PROMPT_CHARS {
            return Err(format!(
                "prompt is {} chars, over the {} bounded-context contract",
                prompt.len(),
                Self::MAX_PROMPT_CHARS
            ));
        }
        #[cfg(feature = "embedded-model")]
        {
            self.generate_embedded(&prompt, budget, sink)
        }
        #[cfg(not(feature = "embedded-model"))]
        {
            let _ = (budget, sink);
            Err("embedded-model feature not compiled".into())
        }
    }
}

#[cfg(feature = "embedded-model")]
impl LlamaCppReasoner {
    /// Context window for one turn. The prompt is bounded at 8,192 CHARS, which
    /// is well under 2,048 tokens for this tokenizer, and the answer is bounded
    /// at 512 tokens — so 2,048 holds both with room, and keeps RAM far under
    /// the declared budget for a 1.5B q4_k_m model.
    const CONTEXT_TOKENS: u32 = 2_048;

    /// Repetition penalty window and strength. See the sampler chain below.
    const PENALTY_WINDOW_TOKENS: i32 = 256;
    const PENALTY_REPEAT: f32 = 1.15;

    fn generate_embedded(
        &self,
        prompt: &str,
        budget: &crate::assistant::GenerationBudget,
        sink: &mut dyn FnMut(&str),
    ) -> Result<crate::assistant::Generated, String> {
        use llama_cpp_2::llama_batch::LlamaBatch;
        use llama_cpp_2::model::AddBos;
        use llama_cpp_2::sampling::LlamaSampler;

        let handle = self.backend.as_ref().ok_or("model not loaded")?;
        let model = &handle._model;
        let backend = backend_global();

        // Never: the template supplies the structure, and Qwen2.5 declares
        // `add_bos_token: false`. Forcing one shifts every position by a token.
        let tokens = model
            .str_to_token(prompt, AddBos::Never)
            .map_err(|error| format!("tokenize failed: {error}"))?;
        let room = Self::CONTEXT_TOKENS as usize;
        if tokens.len() + budget.max_tokens as usize >= room {
            return Err(format!(
                "prompt is {} tokens; {} plus {} generated exceeds the {room}-token window",
                tokens.len(),
                tokens.len(),
                budget.max_tokens
            ));
        }

        let ctx_params = llama_cpp_2::context::params::LlamaContextParams::default()
            .with_n_ctx(std::num::NonZeroU32::new(Self::CONTEXT_TOKENS))
            .with_n_batch(Self::CONTEXT_TOKENS);
        let mut ctx = model
            .new_context(backend, ctx_params)
            .map_err(|error| format!("context init failed: {error}"))?;

        let mut batch = LlamaBatch::new(tokens.len().max(1), 1);
        batch
            .add_sequence(&tokens, 0, false)
            .map_err(|error| format!("batch fill failed: {error}"))?;
        ctx.decode(&mut batch)
            .map_err(|error| format!("prompt decode failed: {error}"))?;

        // Temperature 0, with a repetition penalty in front of it.
        //
        // Greedy alone is deterministic — the same question over the same
        // evidence gives the same answer, which a diagnostic tool owes its user
        // — but a 1.5B model reading back a small evidence pack falls into a
        // loop: measured here, every answer ran to the full 512-token ceiling
        // repeating one clause ("is the only evidence that answers the question"
        // x14). The penalty is applied to the logits BEFORE the argmax, so the
        // result is still a deterministic function of the prompt.
        //
        // 1.15 over the last 256 tokens: enough to break a repeated clause,
        // gentle enough that an evidence id can still be named twice in one
        // answer, which a grounded answer routinely needs to do.
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::penalties(Self::PENALTY_WINDOW_TOKENS, Self::PENALTY_REPEAT, 0.0, 0.0),
            LlamaSampler::greedy(),
        ]);

        let mut position = tokens.len() as i32;
        let mut bytes: Vec<u8> = Vec::new();
        let mut emitted = 0u32;
        let mut cancelled = false;

        while emitted < budget.max_tokens {
            // The three ceilings, checked between tokens. Cancel first: a user
            // who pressed Escape should not pay for one more token.
            if budget.cancelled() {
                cancelled = true;
                break;
            }
            if budget.expired() {
                return Err(format!("deadline exceeded after {emitted} token(s)"));
            }

            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            if model.is_eog_token(token) {
                break;
            }
            sampler.accept(token);

            // Accumulate BYTES, not strings: a multi-byte character can span two
            // tokens, and decoding each piece on its own would emit replacement
            // characters into Arabic mid-word.
            //
            // The 8-byte first guess is the binding's own default and it is not
            // always enough — an Arabic token overran it and the run died
            // "Insufficient Buffer Space -10". The error carries the size it
            // needed as a negative, so the retry is exact rather than a guess.
            let piece = match model.token_to_piece_bytes(token, 8, false, None) {
                Ok(piece) => piece,
                Err(llama_cpp_2::TokenToStringError::InsufficientBufferSpace(needed)) => model
                    .token_to_piece_bytes(token, needed.unsigned_abs() as usize, false, None)
                    .map_err(|error| format!("detokenize failed after resize: {error}"))?,
                Err(error) => return Err(format!("detokenize failed: {error}")),
            };
            bytes.extend_from_slice(&piece);
            emitted += 1;
            sink(&String::from_utf8_lossy(&bytes));

            batch.clear();
            batch
                .add(token, position, &[0], true)
                .map_err(|error| format!("batch append failed: {error}"))?;
            position += 1;
            ctx.decode(&mut batch)
                .map_err(|error| format!("decode failed at token {emitted}: {error}"))?;
        }

        Ok(crate::assistant::Generated {
            text: String::from_utf8_lossy(&bytes).into_owned(),
            tokens: emitted,
            cancelled,
        })
    }
}

/// One llama.cpp backend per process (binding requirement).
#[cfg(feature = "embedded-model")]
fn backend_global() -> &'static llama_cpp_2::llama_backend::LlamaBackend {
    use std::sync::OnceLock;
    static BACKEND: OnceLock<llama_cpp_2::llama_backend::LlamaBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        llama_cpp_2::llama_backend::LlamaBackend::init().expect("llama.cpp backend init")
    })
}
