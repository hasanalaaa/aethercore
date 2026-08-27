# Phase 33 — Compliance Reporting Architecture

## Profile contract

`COMPLIANCE_PROFILE_V1` is a strict, local, identifier-only JSON format. Unknown
fields, unknown schemas, unknown profile IDs, malformed CIS identifiers, empty
controls, duplicate control IDs, duplicate rule mappings, and references to
unimplemented P32 rules are rejected before evaluation. The bundled `cis-l1`
and `cis-l2` assets map every implemented P32 security-audit rule. The `general`
family holds rules without a more natural family so no rule silently vanishes.

The assets reference CIS control identifiers and AetherCore rule mappings only.
Official CIS Benchmark prose is not republished, and this feature does not claim
an official CIS certification.

## Evaluation vocabulary and score

Every profile control produces exactly one of four typed states:

- `Pass`: every mapped lane was available and no failing mapped finding exists.
- `Fail`: an available lane produced a mapped security finding.
- `NotApplicable`: the control does not apply to the current platform.
- `NotVerified`: required evidence was unavailable, denied, missing, or partial.

`NotVerified` is deliberately not `Pass`: lack of evidence cannot establish a
security posture. Evaluation enforces the invariant
`pass + fail + na + not_verified == controls.len()`. The percentage is
`pass / (pass + fail) * 100`; `NotApplicable` and `NotVerified` are excluded
from the denominator. If `pass + fail == 0`, the report contains the typed
`NoVerifiableControls` state instead of a fabricated percentage.

Evidence references contain finding IDs only. Raw facts, observed secret text,
and expected-value strings from security-audit evidence never enter a compliance
report.

## Report, digest, and rendering

The JSON envelope schema is `aethercore.compliance.v1`. Canonical digest input
contains the schema, profile ID, host fingerprint, sorted controls, sorted
evidence references, and score. `generated_unix_ms`, the digest field itself,
the `signed` flag, and the optional signature are normalized out. This makes
the digest stable for the same semantic observation while retaining the actual
point-in-time timestamp in the serialized report.

HTML rendering is a single printable EN/AR document. CSS is inline, Arabic uses
`dir="rtl"`, identifiers remain in code styling, and the renderer contains no
external JavaScript, fonts, CDN, HTTP, or HTTPS resource dependency.

## Point-in-time trust model

Signing reuses the Phase 29 Ed25519 digest primitive with an explicitly supplied
owner seed created through `aetherctl keys generate`. A valid signature proves
that the report digest has not changed and that the signer possessed the private
key at signing time. An unsigned report states `signed: false` and has no fake
signature.

A signed report does **not** prove continuous compliance, correctness or
completeness of every host observation, external certification, or official CIS
certification. Verification is fully offline and distinguishes malformed report,
schema/profile mismatch, score inconsistency, digest mismatch, signed-flag
inconsistency, and signature mismatch.
