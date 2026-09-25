//! Phase 23.1 — PERMANENT embedded model: integrity + real llama.cpp loading.
//!
//! Owner decision: the local AI engine ships EMBEDDED and ENABLED BY DEFAULT.
//! No user-facing flag, no manual placement, no picker. The deterministic fallback
//! is ONLY automatic runtime fault-handling (I3), never a supported configuration.

use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::assistant::{GenerationBudget, MODEL_BUSY};
use crate::engine::LocalReasoner;
use crate::model::{Citation, Insight, InsightConfidence, InsightEngineKind, TypedEvidencePack};
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
///
/// Streamed through the hasher: this used to `fs::read` the whole 1.1 GB
/// artifact into the heap, at every service start, before llama.cpp mapped it.
pub fn verify_model_hash(model_path: &Path, pinned: &ModelManifestEntry) -> Result<(), String> {
    if pinned.declared_ram_budget_bytes > crate::engine::MAX_MODEL_RAM_BUDGET_BYTES {
        return Err(format!(
            "declared RAM budget {} exceeds cap",
            pinned.declared_ram_budget_bytes
        ));
    }
    let mut file =
        std::fs::File::open(model_path).map_err(|e| format!("model read failed: {e}"))?;
    let mut hasher = sha2::Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| format!("model read failed: {e}"))?;
    let digest = hasher.finalize();
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
/// load within budget. Returns the loaded reasoner and the typed startup log
/// line for the caller to emit.
///
/// P75: this is the ONE load. The service used to verify and load the artifact
/// twice — once for insights, once for the assistant — holding two copies with
/// two independent busy lanes, so both could generate at once on a 2-vCPU
/// machine. The reasoner is cheap to clone and every clone shares the one
/// model and the one generation gate, so the insight path and the assistant
/// each take a clone.
pub fn activate_embedded_reasoner(
    product_root: &Path,
) -> Result<(LlamaCppReasoner, String), String> {
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
    Ok((reasoner, label))
}

/// The rules the model writes INSIGHTS under. Separate from the assistant's
/// `SYSTEM_PROMPT` because the output is different: findings the product
/// volunteers, one per line, each machine-checkable against the pack. The shape
/// is enforced by [`insight_grammar`]; this prompt is what makes the model fill
/// that shape with something worth reading.
pub const INSIGHT_SYSTEM_PROMPT: &str = "You are an offline diagnostic assistant for one computer. \
State up to three findings about this computer, one per line, using ONLY the EVIDENCE block. \
Each finding is one short sentence that ends with the tags of the evidence it rests on, in square \
brackets, then a period. Never use general knowledge about computers. Never copy the long ids.\n\n\
Example EVIDENCE:\n\
[E1] surface=MaintenanceHistory id=plan-0007 :: plan plan-0007 domain startup stage completed\n\
[E2] surface=TimelinePattern id=pattern-disk-2 :: recurring failure: class operation domain disk \
code DSK-2, 3 occurrences, recurrence confidence weak\n\
Example ANSWER:\n\
A startup maintenance plan completed [E1].\n\
A disk operation has failed three times [E2].\n\n\
If the EVIDENCE block supports no finding, reply with exactly NO EVIDENCE.";

/// Token ceiling for one insight generation: three short cited lines. A line
/// the ceiling cuts off is incomplete and is dropped, never shown truncated.
pub const INSIGHT_MAX_TOKENS: u32 = 160;

/// The only output shape the insight sampler can produce, built per call from
/// the pack size: "NO EVIDENCE", or one to three lines, each prose ending in one
/// or more `[En]` tags and a period. `idx` enumerates exactly `1..=n`, so a tag
/// naming evidence the pack does not hold cannot be generated at all. Prose
/// excludes brackets and newlines, and allows a period only inside a number
/// (`18.4`), so a line cannot end before its citation.
pub fn insight_grammar(pack_len: usize) -> String {
    let idx = (1..=pack_len.max(1))
        .map(|i| format!("\"{i}\""))
        .collect::<Vec<_>>()
        .join(" | ");
    format!(
        "root ::= \"NO EVIDENCE\" | line line? line?\n\
         line ::= txt cite+ \".\\n\"\n\
         txt ::= ([^\\[\\]\\n.] | \".\" [0-9])+\n\
         cite ::= \" [E\" idx \"]\"\n\
         idx ::= {idx}\n"
    )
}

/// Turns generated insight text into insights. Pure, so every rule below is
/// tested without a model.
///
/// A line is admitted only when it is COMPLETE (ends `.\n` — a line cut off by
/// the token ceiling is not) and EVERY tag on it resolves against the pack. One
/// unresolvable tag drops the whole line rather than the tag: the claim was
/// written as resting on that evidence, and the product does not hold it. The
/// tags leave the explanation, because the renderer shows citations beside it,
/// not a pack index it cannot resolve. One cited observation is Weak, two or more
/// Moderate; a 1.5B model's reading is never Strong.
pub fn insights_from_text(text: &str, pack: &TypedEvidencePack) -> Vec<Insight> {
    text.split_inclusive('\n')
        .filter_map(|line| {
            let (prose, tags) = line.strip_suffix(".\n")?.split_once(" [E")?;
            let mut citations: Vec<Citation> = Vec::new();
            for tag in tags.split(" [E") {
                let index: usize = tag.strip_suffix(']')?.parse().ok()?;
                let item = index.checked_sub(1).and_then(|i| pack.items.get(i))?;
                let citation = Citation {
                    evidence_id: item.evidence_id.clone(),
                    surface: item.surface,
                };
                if !citations.contains(&citation) {
                    citations.push(citation);
                }
            }
            let prose = prose.trim();
            if prose.is_empty() {
                return None;
            }
            let confidence = if citations.len() >= 2 {
                InsightConfidence::Moderate
            } else {
                InsightConfidence::Weak
            };
            Insight::build(
                "insight.summary.observation",
                format!("{prose}."),
                confidence,
                citations,
                InsightEngineKind::LocalModel,
            )
        })
        .collect()
}

/// A system turn and a user turn in Qwen's own chat template. `str_to_token`
/// parses special tokens, so the control tokens are real ones, and `<|im_end|>`
/// is an EOG token — what ends generation naturally.
fn chat_prompt(system: &str, user: &str) -> String {
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n"
    )
}

/// LlamaCpp-backed reasoner — the PERMANENT default engine when the artifact verifies.
///
/// `Clone` shares, it does not copy: every clone holds the same loaded model and
/// the same generation gate. Generation is serialised through that gate with a
/// TRY-lock, so the insight path and the assistant never decode at once on the
/// user's machine and neither ever queues behind the other: the one that finds
/// the gate held returns [`MODEL_BUSY`] at once — an insight then degrades to the
/// rule engine, an assistant turn is refused `Busy`.
#[derive(Clone)]
pub struct LlamaCppReasoner {
    loaded: bool,
    /// Handle text kept opaque; the binding types are feature-internal.
    backend: Option<Arc<LlamaBackendHandle>>,
    gate: Arc<Mutex<()>>,
}

#[cfg(feature = "embedded-model")]
struct LlamaBackendHandle {
    model: llama_cpp_2::model::LlamaModel,
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
            gate: Arc::new(Mutex::new(())),
        }
    }

    /// The insight generation itself, exposed so a test can read its token
    /// count and wall time; [`LocalReasoner::infer`] is this plus
    /// [`insights_from_text`].
    pub fn generate_insight_text(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        deadline: std::time::Instant,
    ) -> Result<crate::assistant::Generated, String> {
        // DBT-P46-B23: an empty pack would ask the model the question with NO
        // evidence, and anything it answered would be uncitable by construction.
        if pack.items.is_empty() {
            return Err("an empty evidence pack has nothing to cite".into());
        }
        let prompt = chat_prompt(
            INSIGHT_SYSTEM_PROMPT,
            &crate::assistant::render_user_message(pack, question),
        );
        if prompt.len() > Self::MAX_PROMPT_CHARS {
            return Err("prompt exceeds bounded context contract".into());
        }
        // Its own cancel flag: nothing outside raises it. The deadline is the
        // selector's, so the insight path keeps its 10 s budget on the RPC thread.
        let budget = GenerationBudget {
            deadline,
            max_tokens: INSIGHT_MAX_TOKENS,
            cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        #[cfg(feature = "embedded-model")]
        {
            self.decode_loop(
                &prompt,
                &budget,
                Some(&insight_grammar(pack.items.len())),
                &mut |_| {},
            )
        }
        #[cfg(not(feature = "embedded-model"))]
        {
            let _ = budget;
            Err("embedded-model feature not compiled".into())
        }
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
                backend_global()?,
                model_path,
                &llama_cpp_2::model::params::LlamaModelParams::default(),
            )
            .map_err(|e| format!("gguf load failed: {e}"))?;

            // Context creation at load proves the artifact loads into a working
            // window; each generation builds its own context from the shared model.
            let backend = backend_global()?;
            let ctx_params = llama_cpp_2::context::params::LlamaContextParams::default()
                .with_n_ctx(std::num::NonZeroU32::new(2048))
                .with_n_batch(256);
            {
                let _ctx = model
                    .new_context(backend, ctx_params)
                    .map_err(|e| format!("context init failed: {e}"))?;
            }

            self.backend = Some(Arc::new(LlamaBackendHandle { model }));
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

    /// P75 — `DBT-P56-002`'s other half. This returned `Err` unconditionally
    /// ("token-level generation requires the context pool…"), so every insight in
    /// the product's life came from the rule engine. It now generates under the
    /// insight grammar and admits only complete, fully cited lines.
    fn infer(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        deadline: std::time::Instant,
    ) -> Result<Vec<Insight>, String> {
        let generated = self.generate_insight_text(pack, question, deadline)?;
        Ok(insights_from_text(&generated.text, pack))
    }
}

/// Phase 56 — real token-level generation for the assistant.
///
/// Greedy rather than sampled because the same question over the same evidence
/// must give the same answer; a diagnostic tool that says something different
/// each time you ask is not one.
impl crate::assistant::StreamingReasoner for LlamaCppReasoner {
    fn is_loaded(&self) -> bool {
        self.loaded
    }

    fn generate(
        &self,
        pack: &TypedEvidencePack,
        question: &str,
        budget: &GenerationBudget,
        sink: &mut dyn FnMut(&str),
    ) -> Result<crate::assistant::Generated, String> {
        // The shipped artifact is qwen2.5-1.5b-INSTRUCT, and an instruct model
        // follows its rules far better inside its own chat template than in a
        // flat prompt. Measured: flat, the model echoed the instruction text
        // back into the answer ("Mark every claim with the tag of the evidence
        // it rests on: NO EVIDENCE") and kept writing past its conclusion.
        let prompt = chat_prompt(
            crate::assistant::SYSTEM_PROMPT,
            &crate::assistant::render_user_message(pack, question),
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
            self.decode_loop(&prompt, budget, None, sink)
        }
        #[cfg(not(feature = "embedded-model"))]
        {
            let _ = (budget, sink);
            Err("embedded-model feature not compiled".into())
        }
    }
}

#[cfg(feature = "embedded-model")]
/// Whether one more decode step (a prompt chunk, or one generated token) can start at
/// `now` and still end by `deadline`. One `decode` cannot be interrupted, and on the
/// 2-vCPU runner a 128-token chunk took over a second: checking only "is it past the
/// deadline yet" overran the insight budget by up to 1.4 s (CI `36185690510`). The
/// previous step's time predicts the next; the first step has no estimate and starts if
/// the deadline has not passed.
fn step_fits(
    now: std::time::Instant,
    last_step: Option<std::time::Duration>,
    deadline: std::time::Instant,
) -> bool {
    now + last_step.unwrap_or_default() < deadline
}

impl LlamaCppReasoner {
    /// Context window for one generation. The prompt is bounded at 8,192 CHARS,
    /// well under 2,048 tokens for this tokenizer, and an answer at 512 tokens —
    /// so 2,048 holds both with room, and keeps RAM far under the declared budget.
    const CONTEXT_TOKENS: u32 = 2_048;

    /// The prompt is decoded in chunks of this many tokens with the ceilings
    /// checked between them. One `decode` call cannot be interrupted, and on a
    /// slow CPU a whole prompt in one call ran long past the deadline it was
    /// handed before the loop below ever looked at the clock.
    const PROMPT_CHUNK_TOKENS: usize = 128;

    /// Repetition penalty window and strength. See the sampler chain below.
    const PENALTY_WINDOW_TOKENS: i32 = 256;
    const PENALTY_REPEAT: f32 = 1.15;

    /// The one decode loop both paths run. `grammar` constrains every sampled
    /// token (insights); `None` leaves the sampler as the assistant has always
    /// had it.
    fn decode_loop(
        &self,
        prompt: &str,
        budget: &GenerationBudget,
        grammar: Option<&str>,
        sink: &mut dyn FnMut(&str),
    ) -> Result<crate::assistant::Generated, String> {
        use llama_cpp_2::llama_batch::LlamaBatch;
        use llama_cpp_2::model::AddBos;
        use llama_cpp_2::sampling::LlamaSampler;

        let handle = self.backend.as_ref().ok_or("model not loaded")?;
        let _held = match self.gate.try_lock() {
            Ok(held) => held,
            // A panic mid-generation poisons the gate. The model is read-only
            // after load and each call builds its own context, so there is no
            // torn state to protect — refusing forever would only disable it.
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return Err(MODEL_BUSY.into()),
        };
        let model = &handle.model;
        let backend = backend_global()?;

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

        // llama.cpp's default is 4 threads whatever the machine. On the 2-vCPU
        // runner that is 2x oversubscription on every decode (DBT-P62-004).
        // ponytail: capped at the old default of 4 so a large machine is not
        // taken over while the user is diagnosing why it is slow.
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(4)) as i32;
        let ctx_params = llama_cpp_2::context::params::LlamaContextParams::default()
            .with_n_ctx(std::num::NonZeroU32::new(Self::CONTEXT_TOKENS))
            .with_n_batch(Self::PROMPT_CHUNK_TOKENS as u32)
            .with_n_threads(threads)
            .with_n_threads_batch(threads);
        let mut ctx = model
            .new_context(backend, ctx_params)
            .map_err(|error| format!("context init failed: {error}"))?;

        let mut batch = LlamaBatch::new(Self::PROMPT_CHUNK_TOKENS, 1);
        let mut last_chunk: Option<std::time::Duration> = None;
        for (chunk_index, chunk) in tokens.chunks(Self::PROMPT_CHUNK_TOKENS).enumerate() {
            let first = chunk_index * Self::PROMPT_CHUNK_TOKENS;
            if budget.cancelled() {
                return Ok(crate::assistant::Generated {
                    cancelled: true,
                    ..Default::default()
                });
            }
            if !step_fits(std::time::Instant::now(), last_chunk, budget.deadline) {
                return Err(format!(
                    "deadline exceeded after {first} of {} prompt token(s)",
                    tokens.len()
                ));
            }
            batch.clear();
            for (offset, token) in chunk.iter().enumerate() {
                let position = first + offset;
                batch
                    .add(*token, position as i32, &[0], position + 1 == tokens.len())
                    .map_err(|error| format!("batch fill failed: {error}"))?;
            }
            let started = std::time::Instant::now();
            ctx.decode(&mut batch)
                .map_err(|error| format!("prompt decode failed: {error}"))?;
            last_chunk = Some(started.elapsed());
        }

        // Temperature 0, with a repetition penalty in front of it.
        //
        // Greedy alone is deterministic, but a 1.5B model reading back a small
        // evidence pack falls into a loop: measured, every answer ran to the full
        // 512-token ceiling repeating one clause. The penalty is applied to the
        // logits BEFORE the argmax, so the result is still a deterministic
        // function of the prompt. 1.15 over the last 256 tokens: enough to break
        // a repeated clause, gentle enough that an evidence tag can be named twice.
        //
        // A grammar goes first, so nothing it forbids reaches the penalty or the
        // argmax.
        let mut chain = Vec::with_capacity(3);
        if let Some(grammar) = grammar {
            chain.push(
                LlamaSampler::grammar(model, grammar, "root")
                    .map_err(|error| format!("grammar rejected: {error}"))?,
            );
        }
        chain.push(LlamaSampler::penalties(
            Self::PENALTY_WINDOW_TOKENS,
            Self::PENALTY_REPEAT,
            0.0,
            0.0,
        ));
        chain.push(LlamaSampler::greedy());
        let mut sampler = LlamaSampler::chain_simple(chain);

        let mut position = tokens.len() as i32;
        let mut bytes: Vec<u8> = Vec::new();
        let mut emitted = 0u32;
        let mut cancelled = false;
        let mut streamed = String::new();

        let mut last_step: Option<std::time::Duration> = None;
        let mut step_started: Option<std::time::Instant> = None;
        while emitted < budget.max_tokens {
            // The three ceilings, checked between tokens. Cancel first: a user
            // who pressed Escape should not pay for one more token.
            if budget.cancelled() {
                cancelled = true;
                break;
            }
            let now = std::time::Instant::now();
            if let Some(started) = step_started {
                last_step = Some(now - started);
            }
            step_started = Some(now);
            if !step_fits(now, last_step, budget.deadline) {
                return Err(format!("deadline exceeded after {emitted} token(s)"));
            }

            // `sample` also ACCEPTS the token into every sampler in the chain
            // (llama_sampler_sample calls llama_sampler_accept). The explicit
            // `accept` this loop used to make on top of it counted each token
            // twice in the penalty window — and a grammar accepting one token
            // twice empties its stacks.
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            if model.is_eog_token(token) {
                break;
            }

            // Accumulate BYTES, not strings: a multi-byte character can span two
            // tokens. The 8-byte first guess is the binding's own default and an
            // Arabic token overran it ("Insufficient Buffer Space -10"); the error
            // carries the size it needed, so the retry is exact.
            let piece = match model.token_to_piece_bytes(token, 8, false, None) {
                Ok(piece) => piece,
                Err(llama_cpp_2::TokenToStringError::InsufficientBufferSpace(needed)) => model
                    .token_to_piece_bytes(token, needed.unsigned_abs() as usize, false, None)
                    .map_err(|error| format!("detokenize failed after resize: {error}"))?,
                Err(error) => return Err(format!("detokenize failed: {error}")),
            };
            bytes.extend_from_slice(&piece);
            emitted += 1;
            // Stream only the complete characters. Decoding the whole buffer
            // lossily put U+FFFD at the end whenever a character straddled two
            // tokens, and the next event replaced it — the stream went BACKWARDS
            // mid-word in Arabic, where it must only ever grow.
            let complete = match std::str::from_utf8(&bytes) {
                Ok(text) => text,
                Err(error) => std::str::from_utf8(&bytes[..error.valid_up_to()]).unwrap_or(""),
            };
            streamed.clear();
            streamed.push_str(complete);
            sink(&streamed);

            batch.clear();
            batch
                .add(token, position, &[0], true)
                .map_err(|error| format!("batch append failed: {error}"))?;
            position += 1;
            ctx.decode(&mut batch)
                .map_err(|error| format!("decode failed at token {emitted}: {error}"))?;
        }

        let text = String::from_utf8_lossy(&bytes).into_owned();
        if text != streamed {
            // Only a trailing partial character was held back; the final text
            // still extends everything streamed.
            sink(&text);
        }
        Ok(crate::assistant::Generated {
            text,
            tokens: emitted,
            cancelled,
        })
    }
}

/// One llama.cpp backend per process (binding requirement).
#[cfg(feature = "embedded-model")]
fn backend_global() -> Result<&'static llama_cpp_2::llama_backend::LlamaBackend, String> {
    use std::sync::OnceLock;
    // `init()` can genuinely fail - it is a native library bring-up - and every caller here
    // already returns `Result<_, String>`, so the failure is reported instead of aborting the
    // service. `OnceLock<Option<_>>` because the backend is not clonable and a failed init
    // must not be retried into a second global. P63.
    static BACKEND: OnceLock<Option<llama_cpp_2::llama_backend::LlamaBackend>> = OnceLock::new();
    BACKEND
        .get_or_init(|| llama_cpp_2::llama_backend::LlamaBackend::init().ok())
        .as_ref()
        .ok_or_else(|| "llama.cpp backend init failed".to_string())
}

#[cfg(test)]
mod tests {

    /// P75 — a decode step (prompt chunk or token) that would end past the deadline does not
    /// start; stopping only once it had passed overshot by 1.4 s (DBT-P62-004).
    #[test]
    fn a_decode_step_that_would_end_past_the_deadline_does_not_start() {
        use std::time::Duration;
        let now = std::time::Instant::now();
        let deadline = now + Duration::from_millis(1_000);
        assert!(step_fits(now, None, deadline), "the first chunk starts");
        assert!(step_fits(now, Some(Duration::from_millis(900)), deadline));
        assert!(!step_fits(
            now,
            Some(Duration::from_millis(1_300)),
            deadline
        ));
        assert!(
            !step_fits(deadline, None, deadline),
            "a passed deadline stops"
        );
    }

    use super::*;
    use crate::model::{EvidenceItem, EvidenceSurface};

    fn pack() -> TypedEvidencePack {
        let mut pack = TypedEvidencePack::default();
        for (id, surface) in [
            ("plan-1", EvidenceSurface::MaintenanceHistory),
            ("pattern-1", EvidenceSurface::TimelinePattern),
        ] {
            pack.push(EvidenceItem {
                evidence_id: id.into(),
                surface,
                detail: format!("detail for {id}"),
            });
        }
        pack
    }

    #[test]
    fn complete_cited_lines_become_insights_without_their_tags() {
        let insights = insights_from_text(
            "A cleanup plan completed [E1].\nA failure recurs [E2] [E1].\n",
            &pack(),
        );
        assert_eq!(insights.len(), 2, "{insights:?}");
        assert_eq!(insights[0].explanation, "A cleanup plan completed.");
        assert_eq!(insights[0].confidence, InsightConfidence::Weak);
        assert_eq!(insights[0].citations[0].evidence_id, "plan-1");
        assert_eq!(insights[1].confidence, InsightConfidence::Moderate);
        assert_eq!(insights[1].citations.len(), 2);
        assert!(
            insights
                .iter()
                .all(|i| i.engine == InsightEngineKind::LocalModel
                    && i.summary_key == "insight.summary.observation")
        );
    }

    /// One tag the pack cannot resolve drops the whole line, not just the tag.
    #[test]
    fn a_line_with_an_unresolvable_tag_is_dropped_whole() {
        let insights =
            insights_from_text("Disk is failing [E1] [E9].\nA plan ran [E1].\n", &pack());
        assert_eq!(insights.len(), 1, "{insights:?}");
        assert_eq!(insights[0].explanation, "A plan ran.");
        assert!(insights_from_text("Ghost [E0].\n", &pack()).is_empty());
    }

    /// A line the token ceiling cut off never finished its claim.
    #[test]
    fn a_truncated_last_line_is_dropped() {
        let insights = insights_from_text("A plan ran [E1].\nA failure rec", &pack());
        assert_eq!(insights.len(), 1);
        assert!(insights_from_text("A plan ran [E1]", &pack()).is_empty());
        assert!(insights_from_text("A plan ran [E1].", &pack()).is_empty());
    }

    #[test]
    fn the_models_refusal_and_uncited_prose_yield_nothing() {
        assert!(insights_from_text("NO EVIDENCE", &pack()).is_empty());
        assert!(insights_from_text("Your PC is slow.\n", &pack()).is_empty());
        assert!(insights_from_text(" [E1].\n", &pack()).is_empty());
    }

    #[test]
    fn arabic_findings_keep_their_text() {
        let insights = insights_from_text("اكتملت خطة تنظيف [E1].\n", &pack());
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].explanation, "اكتملت خطة تنظيف.");
    }

    /// The grammar enumerates exactly the pack's tags.
    #[test]
    fn the_grammar_names_every_tag_the_pack_holds_and_no_other() {
        let grammar = insight_grammar(3);
        assert!(
            grammar.contains("idx ::= \"1\" | \"2\" | \"3\"\n"),
            "{grammar}"
        );
        assert!(!grammar.contains("\"4\""), "{grammar}");
        assert!(grammar.starts_with("root ::= \"NO EVIDENCE\" | line line? line?"));
    }
}
