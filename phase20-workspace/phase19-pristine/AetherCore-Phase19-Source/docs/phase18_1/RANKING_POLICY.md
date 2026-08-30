# Driver Candidate Ranking Policy

Ranking applies only to **concrete driver candidates**, never to generic management utilities.

Deterministic precedence:

1. trust state
2. applicability
3. authority context for the current machine/device
4. device specificity
5. machine specificity
6. version/date when meaningful
7. stable candidate identity tie-break

Context does not override trust/applicability. A higher-version generic package does not automatically outrank a trustworthy machine-specific candidate, and a management utility is not numerically compared against a package.

The executable source tests cover unsigned rejection, OEM vs generic context, self-built component context, multiple official candidates, deterministic tie-breaking, and trust-before-context. `tests/fixtures/phase18_1/execution-mapping.json` binds D18 scenarios to assertion-bearing Rust tests.
