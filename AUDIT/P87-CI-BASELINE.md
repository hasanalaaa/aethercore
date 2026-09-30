# P87-01 CI baseline

Measured 2026-09-30 (UTC timestamps from GitHub Actions jobs/steps API).
Parent: `e685b14f4290ee7b7de64b15c187a4a9453b521e`.

The last five successful CI runs all used Rust 1.97.1, Node 22.16.0, pnpm 11.22.0,
and the project self-hosted Windows PC (`aether-win` / `aether-win-2`).
Push and same-repository PR runs use the same gates. SHAs differ only across
P78 changes; this is a comparable recent baseline, not a controlled benchmark.

## Before

| Run / head | Windows queue s | Windows execution s | ADK s | Rust tests s | UI check + units s | UI feedback from run creation s |
|---|---:|---:|---:|---:|---:|---:|
| [36658277708](https://github.com/hasanalaaa/aethercore/actions/runs/36658277708) / `e685b14f4290ee7b7de64b15c187a4a9453b521e` | 3 | 2529 | 1 | 560 | 20 | 664 |
| [36654400180](https://github.com/hasanalaaa/aethercore/actions/runs/36654400180) / `c5cca5a447e4e24a52459be29ad25ab85f950ae1` | 3 | 2865 | 1 | 500 | 6 | 559 |
| [36653984224](https://github.com/hasanalaaa/aethercore/actions/runs/36653984224) / `fca43da3703e194fe93d0af68b611321a896a7e2` | 3 | 2899 | 1 | 524 | 9 | 593 |
| [36649220657](https://github.com/hasanalaaa/aethercore/actions/runs/36649220657) / `b6a1fef846c3ba71be38906f6e9e83f0a580ce71` | 3 | 3460 | 0 | 629 | 12 | 696 |
| [36649046176](https://github.com/hasanalaaa/aethercore/actions/runs/36649046176) / `df284239e9f5f4a23da9e86e6f67357e775fcaab` | 3 | 2387 | 1 | 396 | 23 | 460 |

Windows execution median: **2865 s**. Existing UI feedback median: **593 s**.

All five used the persistent Windows target tree and model restore path. They
are warm-path runs; a restored target path is not proof every crate was cached.
Cold compilation and the compile/test split inside `cargo test` are **unmeasured**;
job/step API exposes the combined step only. ADK was 0–1 s (already installed).
Packaging (860–1324 s), recursive qualification (567–1083 s), and artifact upload
(282–441 s) dominate total execution; no target cache changes are justified here.

Raw jobs/steps can be reproduced without relying on this summary:

```sh
gh api repos/hasanalaaa/aethercore/actions/runs/<run-id>/jobs?per_page=100
```

## Change and validation

`fast-ui` runs the existing locale audit, accessibility/type check and all seven
explicit UI unit files independently on Ubuntu, with the pinned Node/pnpm and
frozen lock. Windows keeps native tests, source seal, dependency freeze, UI build,
and all recursive and packaging gates. The rendered-DOM leak gate is unchanged.
`CI required` runs even when dependencies fail/skip/cancel; all six must succeed.
No gate, timeout, dependency or cache policy is relaxed.

New stdlib regression tests on the parent failed for the intended reasons:
UI checks wait behind native tests and the required aggregate is absent.
After: the aggregate shell command rejects each individual failure, cancellation
and skip, rejects missing gates, and accepts six successes.

Branch protection API returned HTTP 404 (main not protected). The stable aggregate
is provided per D24; enabling only `CI required` as a branch-required status needs
the owner/project policy. Manual lane4 merges still require full CI at head SHA.

## After

Pending the PR CI receipt. No speedup is claimed before measured fast-job timing.
Proposed acceptance: fast feedback ≤300 s and ≥25% below the before median.
PR #83 negative-control head `92a89d40f0cab9e21d976b22f497beeb371a1cb8`,
run [36666378019](https://github.com/hasanalaaa/aethercore/actions/runs/36666378019):
`fast-ui` rejected the deliberately missing Arabic navigation keys at 03:53:09Z,
16 s after run creation, before the queued Windows native suite. The temporary
checkout-only injection is removed in the following commit. Controlled cold timing
remains unmeasured.

[Aggregate scheduling semantics](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idneeds).
