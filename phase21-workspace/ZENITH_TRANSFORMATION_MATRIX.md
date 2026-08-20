# AetherCore Zenith Transformation Matrix

The Zenith pass is additive to the existing Phase 0–16 and Enterprise convergence work. It deliberately avoids changing service authority, cryptography, update trust, mutation ownership, persistence, or diagnostic truth semantics where no measurable defect was proven.

| Issue | Area | Defect | Root cause | Implemented correction | Verification | Final state |
|---|---|---|---|---|---|---|
| ZEN-001 | Fluid dialogs | Duplicate/spurious `onClosed` possible under reduced motion | Completion existed in both spring subscriber and reduced-motion branch | Idempotent `completeClose()` with `rendered` guard; one callback callsite | Zenith structural regression; inherited UI build/check on Windows | Fixed |
| ZEN-002 | Pointer press | Drag-away visual cancellation could still produce native activation | Pointer capture changed event targeting but activation was not fenced | One-shot capture-phase click cancellation when pointer-up is unarmed; keyboard path preserved; option updates supported | Zenith structural regression; Windows pointer test required | Fixed |
| ZEN-003 | Direct manipulation | Cancellation inherited fling velocity | Cancellation reused intentional release path | Abort/lost-capture uses zero release velocity; real pointer-up preserves momentum | Zenith regression + Windows gesture fault injection | Fixed |
| ZEN-004 | System Care | Channel state not announced; update progress duplicated non-semantic width bar | Visual state and progress implementation bypassed design primitives | `aria-pressed` and shared `ProgressBar` using ARIA + transform spring | Zenith regression + Svelte check/build | Fixed |
| ZEN-005 | Drivers | Filter selection not announced | Visual `.active` class only | `aria-pressed` on filter toggles | Zenith regression + accessibility qualification | Fixed |
| ZEN-006 | Diagnostics RTL | Right arrow hard-coded into action content | Spatial indicator was treated as text | CSS-generated mirrored direction cue | Zenith regression + Arabic visual matrix | Fixed |
| ZEN-007 | Progress RTL | Transform origin not explicitly mirrored | Physical progress direction lacked engine-independent RTL rule | Explicit left origin in LTR, right origin in RTL | Zenith regression + WebView2 visual matrix | Fixed |
| ZEN-008 | Verification | New findings had no permanent regression gate | Enterprise audit predates Zenith defects | `zenith-adversarial-audit.py/.ps1`, `verify-zenith.ps1`, CI/release/GA integration | Executed Python gate here; PowerShell/native path prepared | Fixed |
| ZEN-009 | Startup | Mutually exclusive decisions had visual-only selected state | Custom button group lacked choice semantics and grouped keyboard behavior | ARIA `radiogroup`/`radio`, `aria-checked`, roving tab stop, RTL-aware Arrow/Home/End navigation | Zenith source regression + Windows accessibility matrix | Fixed |

## Preserved invariants

- Desktop stays non-elevated and the privileged service remains the sole machine-mutation authority.
- `MutationSupervisor`, immutable plans, consent binding, durable checkpoints, update signing/Authenticode/staging authority, support-bundle redaction/integrity, IPC principal partitioning, and bounded queues remain unchanged.
- No new health score, confidence score, SMART interpretation, driver-risk probability, or causal diagnostic claim was introduced.
- No new background mutation authority or renderer polling was introduced.
