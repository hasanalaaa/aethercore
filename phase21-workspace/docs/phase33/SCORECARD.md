# Phase 33 Qualification Scorecard

| Area | Acceptance evidence |
|---|---|
| Strict profiles | Both bundled assets parse; malformed/unknown/duplicate inputs reject typed |
| P32 coverage | Every implemented security-audit rule is mapped in each bundled profile |
| Honest evaluation | Exact Pass/Fail/NotApplicable/NotVerified matrix and totals invariant |
| Scoring | NA/NV excluded; zero verifiable controls returns a typed state |
| Determinism | Same semantic input yields the same digest; fixed timestamp renders byte-stably |
| Signing | Phase 29 primitive reuse, signed pass, digest tamper fail, signature tamper fail |
| Unsigned honesty | Integrity verification passes with `signed:false` and no signature |
| Rendering | JSON schema pinned; HTML nonempty, printable, bilingual, RTL, and air-gapped |
| CLI/i18n | Offline generation/verification; EN/AR parity and selection precedence retained |
| Wire contract | Request maximum 90 and response maximum 52; no proto bytes changed |
| Delivery | Patch/full-tree pass and two independent sealed-P32 reconstructions compare exactly |
| Archive | Two byte-identical builds and external-only final SHA pointer |

The standardized final status report is the authoritative record of exact gate
counts and outputs. This scorecard defines the evidence required and does not
replace execution of any gate.
