# Phase 18.1 Final Adversarial Review

The source review attempted to force each prohibited false statement.

| Attack | Expected | Source result |
|---|---|---|
| Utility available, no update evidence | no recommendation/count | fail closed by structural separation |
| WUA unavailable | no UpToDate | unknown/offline state |
| Required OEM/component authority manual/unavailable | incomplete coverage | strong UpToDate withheld |
| Valid signature, wrong publisher | reject | `UnexpectedPublisher` |
| Signature valid but signer identity unavailable | reject for publisher-sensitive acquisition | fail closed |
| Provider A exact-version ignore applied to Provider B | Provider B visible | provider-scoped suppression |
| Blank-provider ignore override | suppress nothing | fail closed |
| Malformed provider registry | reject | registry validation error |
| Self-built/OEM uncertainty | avoid unsafe OEM certainty | tri-state machine context |
| Legacy vendor utility finding text | must not imply an update | retained only for lifecycle compatibility; wording neutralized |

Source semantic audits and static validation report no remaining High/Critical source defect in Phase 18.1 scope. Windows-native runtime behavior is explicitly outside this proof.
