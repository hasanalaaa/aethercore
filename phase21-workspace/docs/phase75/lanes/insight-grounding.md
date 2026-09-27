# P75 lane `insight-grounding` (part 3, trial run) — evidence

Found by generating insights from the real embedded model (release, Metal) over the product-shaped evidence pack: `cargo test -p aethercore-intelligence-core --release --test embedded_generation -- --nocapture`.

| row | commit | what was wrong | now | red-before |
|---|---|---|---|---|
| `DBT-P75-069` | `dd9bcbb` | every tag resolved, but two of three insights swapped the counts of the patterns they cited ("3 occurrences … [E7]" where E7 holds 5; "5 occurrences [E8]" where E8 holds 3) | a line is admitted only when every number it states is held by the evidence it cites | `a_line_whose_numbers_its_evidence_does_not_hold_is_dropped`: `left: 3, right: 1`; real model after: 1 insight (was 3), all 7 embedded tests pass |

Residual (recorded, not mechanically checkable): the surviving line calls one failed Startup plan "a recurring failure".
