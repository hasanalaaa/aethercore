# AetherCore — Phase 28 Scorecard

| Gate | Criterion | Status |
|---|---|---|
| G0-a baseline verify_phase27 | PASS / 48 checked | ✅ |
| G0-b whole-tree anchor | workspace == sealed snapshot digest `64bd2817f79f342e…` (after normalizing 2 benign drift files + pycache to the sealed archive) | ✅ |
| G0-c baseline adversarial-audit | 421 checks, failures=[] (p26 intermediate FAIL = documented filter signature, untouched) | ✅ |
| G0-d baseline cargo test ×2 | 394 passed / 0 failed, twice identical | ✅ |
| T1 aetherctl crate | hand parser, versioned envelope w/ deny_unknown_fields parse-back, ASCII tables, no clap | ✅ |
| T2 transport reuse | UnixSocketSession + set_io_timeouts (unix); SessionClient reuse (windows, frozen); Reachable{pid}/Offline/StaleEndpointRecovered | ✅ |
| T3 embedded offline set | about/version/capabilities/engine-source/telemetry-once(real PerfPlatform)/self-check(streaming sha256 fail-closed)/detect; mutation-guard scan clean | ✅ |
| T4 service-backed set | doctor/perf/optimize/timeline/care/insights/scan over EXISTING tags; consent discipline enforced | ✅ |
| T5 exit-code registry | enum+docs+tests tri-equality (0/2/3/4/5/6/7/8/130) | ✅ |
| T6 lifecycle closure | SIGTERM drain→cleanup→exit0 proven live; rotated --daemon logs; plutil -lint OK; systemd static keys; units --print writes nothing | ✅ |
| T7 proof suite (real daemon, this Mac) | 6/6 green incl. kill -9 stale-recovery rebind + SIGTERM graceful + consent adversarial | ✅ |
| T8 audit superset + docs | PASS 572 checks (>421); renderer byte-untouched vs sealed P27 | ✅ |
| GB workspace tests ×2 | plain **409 passed / 0 failed** ×2 identical; e2e variant **420 passed / 0 failed** ×2 identical | ✅ |
| GG binary-safe patch round-trip ×2 | applied 30 files onto sealed P27 snapshot → tree byte-identical ×2 (patch bundle itself excluded per P27 convention) | ✅ |
| GH deterministic master zip ×2 | identical ×2; SHA-256 in ../PHASE28_FINAL_SHA256.txt | ✅ |

## Honest limits recorded
- Windows named-pipe CLI lane: reused frozen code, no Windows host proof (QD-028-001).
- systemd runtime verification NOT_EXECUTED without Linux (QD-028-002).
- CLI strings EN-only by documented decision (QD-028-004); desktop stays EN/AR.
- svelte-check: 0 errors / 17 warnings unchanged (renderer untouched, byte-compared).
