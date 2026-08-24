# Phase 23.1 — Permanent Embedded Model

Owner decision: the local AI engine ships EMBEDDED and ENABLED BY DEFAULT. The
deterministic fallback is ONLY automatic runtime fault-handling (I3) — never a
supported configuration, never user-selectable.

## Artifact

| Field | Value |
|---|---|
| Source repo | `Qwen/Qwen2.5-1.5B-Instruct-GGUF` (official Qwen, Hugging Face) |
| File | `qwen2.5-1.5b-instruct-q4_k_m.gguf` |
| Size | 1,117,320,736 bytes (1.0406 GiB — within the 0.9–1.1 GB binary-unit window) |
| SHA-256 | `6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e` |
| Cross-check source | Hugging Face repo LFS OID for the same file (`/api/models/.../tree/main`, `lfs.oid`) |
| Quantization | q4_k_m |
| License | Apache-2.0 (full text at `assets/models/licenses/Apache-2.0.txt`; Qwen LICENSE copy alongside) |
| Placed at | `assets/models/qwen2.5-1.5b-instruct-q4_k_m.gguf` |

Download used the `/resolve/main/` endpoint during THIS build (build-time network is
allowed; runtime stays air-gapped). The computed SHA-256 equals the repository's
published LFS sha256 exactly.

## Manifest

`assets/models/models.manifest.json` carries the single artifact entry with
`fileName / bytes / sha256Hex / declaredRamBudgetBytes (2147483648) / sourceUrl /
quantization "q4_k_m" / license "Apache-2.0"`. The Phase 23 `activationContract`
section was removed entirely.

## Startup flow (every service start)

1. Locate `assets/models/qwen2.5-1.5b-instruct-q4_k_m.gguf` under the product root.
2. Verify SHA-256 against BOTH the compiled-in pin (`embedded_model_entry()`) and the
   manifest entry — fail-closed on any byte difference.
3. RAM-budget check against `MAX_MODEL_RAM_BUDGET_BYTES` (2 GiB).
4. Load through llama-cpp-2 with a bounded 2048-token context; warm-up context
   creation proves the artifact and window work.
5. Emit exactly one typed log line:
   `intelligence-core: embedded reasoner active (model=qwen2.5-1.5b-instruct-q4_k_m, sha256 ok)`

## Tamper / fault behavior

- Any verification or load failure logs
  `intelligence-core: embedded … FAILED (…); degraded to rule fallback — defect`,
  flips `EMBEDDED_ENGINE_ACTIVE=false`, and the Insights panel chip reads
  **ruleFallback** until the fault clears. Missing/corrupt model in the packaged
  product is a DEFECT, not a supported mode.
- Inference faults mid-run (timeout/malformed output/OOM) degrade that single request
  to fallback silently per invariant I3; no AI-layer error ever reaches the UI.
- No user-facing toggle exists anywhere; internal constants only.

## Packaging note

The ~1 GiB artifact flows through the binary-safe patch (`new-files/`), the
MANIFEST hash set, and the deterministic archive. Package size grows by ≈1.07 GB;
rebuild determinism was re-proven including the model (see SCORECARD.md).
