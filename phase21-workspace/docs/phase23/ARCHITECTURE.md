# Phase 23 — Embedded Local Intelligence Core (Air-Gapped, Advisory-Only)

Status: source-complete on the development host; model-artifact commitment and
Windows CPU-inference qualification remain honest open debt. Part A of this phase
closed QD-022-002 (care domain dispatch) — see §1.

## 1. Part A — closing QD-022-002

The `DomainDispatch` trait now lives in the care-orchestrator crate so service code
and tests share one shape:

```rust
pub trait DomainDispatch: Send + Sync {
    fn start_and_await(&self, owner: &str, plan_id: &str)
        -> Result<(String, String, String), String>; // (plan_state, verification, failure_key)
}
```

Composition provides `RealDomainDispatch` at startup: it probes which coordinator
owns a plan id (cleanup → startup → repair), acquires that domain's own mutation
lease through the kernel supervisor, calls its real `start_with_lease`, then polls
status to a terminal state (`Completed | Failed | RebootPending`) under
`DISPATCH_TIMEOUT_MS` with a 100 ms cadence. Verification citation per domain:
cleanup = all items `result_code == "Deleted"`; startup = all items `"Verified"`;
repair = the coordinator's own `verification_state`. `ServiceExecutor` maps outcomes
onto the unchanged truth-first vocabulary (`VerifiedByDomain`,
`CompletedUnverified`, `Failed`). No router handler behavior changed.

## 2. The six invariants in this delivery

| # | Invariant | Enforcement |
|---|---|---|
| I1 | Advisory-only | Output vocabulary is exactly [`Insight`]; crate deps contain no operation-kernel/mutation/planner surfaces (allowlist gate); `deny_unknown_fields` schema rejects smuggled action fields; destructive-API gate extended to the crate |
| I2 | Mandatory citations | `Insight::build` refuses zero citations; engine drops any insight whose citations don't resolve against the served pack; adversarial test feeds fabricated + stale ids → zero survive |
| I3 | Deterministic fallback | Unavailable/slow/unparseable model path degrades silently to `DeterministicFallbackReasoner`; identical citation discipline; deterministic byte-identical output for identical packs; UI never sees an AI-layer error |
| I4 | Resource budget | `MAX_EVIDENCE_ITEMS=64` bounded pack, detail ≤256 chars, `INFERENCE_TIMEOUT=10s`, single in-flight lane (own `AtomicBool` CAS, NOT MutationWorkload), observer-effect guard refusing inference while mutations run (selector + router both enforce) |
| I5 | Air-gapped by construction | Artifact manifest `assets/models/models.manifest.json` with pinned sha256; `verify_model_hash` fails closed on any byte difference or RAM-budget overflow; zero network symbols/deps (gate a/b) |
| I6 | Audit surface extension | phase23 audit = strict superset (242 → **324** checks); gates listed below |

## 3. Threat model (honest)

**Prompt injection via hostile counter values or finding text.** Evidence packs are
pre-typed structured data: each item is `{evidence_id, surface, detail}` where
`detail` is composed from typed domain fields (state/stage/result codes), truncated
to 256 chars. There is no free-form attacker-controlled prose channel into the
prompt, and insight output is parsed against a strict versioned schema with unknown
fields rejected — a jailbroken response could not introduce new wire semantics.

**Residual risks, stated plainly:**
1. Detail strings include plan ids and stage words that originate locally; a local
   attacker who can already write our SQLite journal has stronger levers than prompt
   injection. Mitigation remains structural typing, not trust.
2. When the LLM path is activated (QD-023-001), hallucinated citations are dropped
   by I2, but a *plausible-but-wrong* explanation over REAL citations cannot be
   detected mechanically. This is why insights render as advisory cards with visible
   confidence classes and engine labels — never as facts, never as actions.
3. Model artifacts are hash-pinned, but the supply chain upstream of the pinned hash
   (the quantized model's training data) is outside this repository's control.

## 4. Model selection decision

Default enabled engine: **DeterministicFallbackReasoner** (rule-based summaries over
bottleneck roles, repair verification states, recurrence patterns). The on-device
LLM path is fully coded behind `--features local-model` with documented manual
activation: candidate artifact class is **Qwen2.5-3B-Instruct GGUF q4_k_m
(Apache-2.0)**, ≤2 GB, strong structured-JSON compliance; reasoning happens in
English internally while summaries are template-localized EN/AR by renderer catalogs.
No binary was fabricated: committing an artifact requires placing it under
`assets/models/`, pinning its sha256 in the manifest, and adding its license file.

## 5. Wire & service integration (additive)

EventKind **27** `EVENT_KIND_INSIGHTS`; envelope payload **36** `insights`;
requests **84–86** (list / request / dismiss); response **48**
`insights_response`. `IntelligenceCoordinator` composes evidence packs from public
read APIs only (maintenance executions + timeline history), keeps insights as
EPHEMERAL session state (never persisted), publishes through the ordered bus, and
labels every response with the serving engine.

## 6. Renderer

Typed contracts + stream slice; `InsightsPanel` mounted on Activity/Care and Overview
routes. "Explain this" runs on-demand inference; badge always reads "AI advisory ·
local · offline · <engine>"; every card shows confidence class and tappable citation
chips resolving to underlying evidence ids (TechnicalText bidi isolation). Loading
uses a reduced-motion-aware progress bar; panel dismiss clears interest. svelte-check:
0 errors / 17 warnings unchanged.

## 7. Verification summary

Baseline 684/242/353 reproduced pre-change. Post-change: workspace **367 passed /
0 failed**, audit chain 43→159→242→**324** PASS, network-ban grep clean, tamper test
green (one flipped byte → loader refuses → fallback serves), svelte-check 0/17,
binary-safe round-trip ×2 byte-identical, master archive deterministic ×2.
