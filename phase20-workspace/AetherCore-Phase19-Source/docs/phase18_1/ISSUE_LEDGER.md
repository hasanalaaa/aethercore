# Phase 18.1 Issue Ledger

| ID | Severity at discovery | Closure | Evidence |
|---|---|---|---|
| P18.1-001 Utility modeled as update candidate | High | Closed at source-model level | management authority separated from `DriverCandidateV2`; GPU no-update regressions |
| P18.1-002 Provider coverage too coarse | High | Closed at source-model level | per-device required/evaluated authority coverage and strong UpToDate gate |
| P18.1-003 Ranking/test contradiction | High | Closed | utilities removed from package ranking; deterministic candidate precedence/tests |
| P18.1-004 Expected publisher not enforced | High | Closed at source-contract level | typed signer evidence, expected signer validation, wrong-publisher fail-closed test |
| P18.1-005 Ignore provider scope ambiguous | Medium | Closed | provider-scoped exact-version suppression + fail-closed blank provider test |
| P18.1-006 OEM context binary/fragile | Medium | Closed at source-model level | `MachineKind::{Oem,SelfBuilt,Unknown}` and conservative classification |
| P18.1-007 Deep Scan management/update conflation | Medium | Closed | separate informational management/coverage/unknown findings |
| P18.1-008 EN/AR remediation parity gap found during regression | Medium | Closed | three new remediation keys added to both catalogs; Phase 17 audit restored PASS |

No Windows-native PASS is claimed. Native signer extraction, live provider behavior, authority completeness, and staging attacks remain qualification debt rather than being mislabeled as closed runtime proof.
