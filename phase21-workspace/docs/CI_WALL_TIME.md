# CI wall time of the Windows jobs

`ci.yml` used to run one `windows` job that built, tested and packaged the release candidate. #122 split it into `windows` (verify and test) and `windows-candidate` (build the candidate, upload it only off pull requests) so they can run on different self-hosted runners at once. This note records what was measured, from `gh api repos/hasanalaaa/aethercore/actions/runs/<run>/jobs` (`started_at` / `completed_at`, queue time excluded).

| run | shape | runner(s) | `windows` | `windows-candidate` | wall | job-minutes |
|---|---|---|---|---|---|---|
| `37294649134` | before, one job | aether-win-3 | 43.4 min | | 43.4 min | 43.4 |
| `37300914874` | before, one job | aether-win | 46.9 min | | 46.9 min | 46.9 |
| `37312353911` | after, serial (same runner, others busy) | aether-win | 14.2 min | 15.0 min | 29.2 min | 29.1 |
| `37998996178` | after, overlapped (PR #135) | aether-win, aether-win-3 | 16.0 min | 18.9 min | **18.9 min** | 34.9 |

In `37998996178` both jobs started at `2026-10-09T22:24:03Z` on different runners; `windows` ended at `22:40:02Z` and `windows-candidate` at `22:42:58Z`. The overlap is proven by the two runner names and the intersecting intervals, not inferred.

## Reading it

- Wall time of the Windows part fell from 43-47 min to 18.9 min when the jobs overlap, and to 29.2 min when they cannot (one runner).
- CPU cost did not fall: 34.9 job-minutes against 29.1 serial, because the three runners are one PC and two builds share its cores and disk (`windows-candidate` took 18.9 min against 15.0 min alone). The split buys latency, not capacity.
- One overlapped sample on an otherwise idle pool. It is a measurement, not a guarantee: with other runs queued the two jobs share a runner and the serial figure applies.
