# Resolution Authority Model

| Finding family | Required scope(s) | Resolution policy |
|---|---|---|
| Driver missing / device problem / driver offer | `drivers` | Successful authoritative absence |
| Windows integrity | `windows` | Successful scope + same-resource healthy fact |
| Storage / memory | `diagnostics` | Successful scope + same-resource healthy fact where state-based |
| WHEA / crash | `diagnostics` | Successful authoritative absence from the bounded event window |
| Startup | `startup` | Successful scope + healthy footprint fact |
| Cleanup | `cleanup` | Successful authoritative absence |
| AetherCore update | `updates` | Successful scope + no-update healthy state |
| Possible driver regression | `drivers` + `diagnostics` + `driver-history` | All scopes successfully reevaluated and correlation absent |

Only exact `CollectorState::Completed` grants resolution authority. `CompletedWithWarnings` deliberately does not: partial enumeration is not negative proof.
