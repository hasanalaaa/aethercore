# AetherCore — Phase 27 Scorecard

| Gate | Criterion | Status |
|---|---|---|
| Baseline verify_phase26 | PASS / 749 checked | ✅ |
| Baseline adversarial-audit | 349 checks, failures=[] | ✅ |
| Baseline cargo test | 388 passed ×2 identical | ✅ |
| T1 UDS composition | unix-ipc builds, same router, one wire contract | ✅ |
| GD live round trip | caps+ping+engine-source over real socket; stale-path recovery; 0700/0600 | ✅ |
| T2 macOS provider | libc-only, 13 unit+live tests green, GPU/thermal honestly Degraded | ✅ |
| T3 Linux provider | typed /proc parsers + fixture tests green | ✅ |
| T4 selection+real-sample | GE green (12 real ticks → aggregate → analyze) | ✅ |
| T5 renderer+i18n | AboutPanel chips + engine source; svelte-check 0 errors / 17 warnings (unchanged) | ✅ |
| T6 audit superset | >349 checks incl. honesty code-backing gate | ✅ |
| GB workspace tests | exact counts in status report (×2 identical) | ⏳ final run |
| GG binary-safe patch | round-trip byte-identical ×2 | ⏳ build |
| GH deterministic zip | SHA-256 identical ×2 | ⏳ build |

## Honest limits recorded
- macOS provider proven on Apple Silicon only (QD-027-001).
- Linux provider proven via fixture tests on this host; live kernel matrix open (QD-027-002).
- SO_PEERCRED deepening deferred (QD-026-001 unchanged).
