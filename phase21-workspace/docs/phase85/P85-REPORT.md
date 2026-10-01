# P85 — supported repair and bounded cleanup

Implementation: task work complete; phase acceptance remains pending the integration commit's full Windows CI and the qualification gaps below. No live repair or cleanup was run on the owner's host.

## Changes and evidence

| Task | Implemented behavior | Verification |
| --- | --- | --- |
| P85-01 | CBS verdicts use the opened file's identity and pre-run offset. Rotation, truncation, unreadable baseline, oversized data, progress alone, incomplete transactions and nonzero exit produce Unknown. A complete fresh transaction is required. | CBS fixtures: 9/9 on macOS and Windows; the incomplete-transaction regression failed before the fix. |
| P85-02 | DISM Restore uses the API, LimitAccess=TRUE, a cancel event, typed HRESULTs and callback-derived percentages only. Sessions share a process-wide lifecycle lock; close/shutdown cannot overlap a live call. The existing two-hour deadline signals cancellation and retains the lock until the API returns. | Percentage/source/cancellation fixtures and session overlap test pass; native Windows compile passes for the repair crate. |
| P85-03 | Output tails are bounded while reading. SFC uses elapsed/stage/indeterminate progress. The existing D3 cancel request is bound through desktop and the connected UI. A request prevents the next repair/verification step. Mutating children are never force-killed for cancellation or deadline; stopped/unknown outcomes require recovery review. | Actual harmless child fixtures on macOS prove ownership is retained until exit; coordinator fixtures cover pre/post-barrier cancellation and late success. UI regression suite: 59/59. |
| P85-04 | A stopped service alone is insufficient to plan a start. A failed dependent update diagnosis is required. Live wuauserv configuration distinguishes automatic, demand-start and disabled; demand-start idle is healthy, disabled policy is preserved, and eligibility is rechecked before StartServiceW. | Diagnosis and service-start-type regressions failed before fixes; repair-intelligence synthetic lab: 34/34 on both platforms. |
| P85-05 | The authenticated peer's existing OS-resolved profile roots reach the cleaner worker. No C:\\Users enumeration occurs. Passive/autonomous scans receive no personal roots. Windows temp retention remains 48 hours; caller temp remains seven days; shader cache remains optional, with recompilation/stutter disclosure. Crash reports/dumps are retained while unresolved-incident linkage is unavailable, with an explicit warning. | Owner A/B coordinator regression; native Windows frozen-evidence fixtures prove an added hardlink and an open file are skipped. Both native regressions failed before single-link validation/exclusive deletion handles. |

## Existing action boundaries

| Action | Required evidence and prerequisites | Consent | Postcheck |
| --- | --- | --- | --- |
| DISM component repair | Repairable component-store diagnosis, fresh DISM preflight, no pending reboot/servicing conflict, durable mutation barrier and existing protection rules | Existing immutable plan authorization | Fresh API CheckImageHealth; API return alone is not verification |
| SFC repair | Protected-file diagnosis, component prerequisites, same barrier/protection rules | Existing immutable plan authorization | Fresh verify-only run and conservative CBS window |
| Start wuauserv | Failed update diagnosis plus current stopped automatic service; never demand-start idle or policy-disabled | Existing immutable plan authorization | Existing service-running verification |
| CHKDSK online scan | Specific filesystem diagnosis; hardware faults continue to require backup guidance | Existing plan rules, no offline repair permission inferred | Read-only /scan; no /f, /r, scheduled repair or reboot |
| WinRE guidance | Unsupported/localized state stays Unknown; executable presence never proves configured recovery | Guidance only | No fabricated recovery-ready result |

## Validation

The task checks ran against local source and a dirty-source Windows overlay, not a clean claimed integration head. An exact committed checkout and source-seal check are required next, followed by root-owned phase CI.

- macOS: locked tests for cleaner, system-repair, repair-intelligence, maintenance-service and pc-intelligence passed; Clippy all-targets with warnings denied passed for those packages.
- Windows (two build jobs, fixture-only): cleaner 14 passed, one live inventory test ignored; system-repair 38 passed, one live assessment ignored; repair-intelligence 34 passed. No ignored live test was counted as acceptance evidence.
- UI: pinned pnpm frozen-lockfile install; source regression tests 59/59; Svelte check zero errors/warnings; production build passed (existing bundle-size warning remains).
- Localization audit 34/34; enterprise adversarial audit 88/88; scheduler audit 63/63. Static validation 351 checks passed, with YAML parsing explicitly unmeasured on local Python because PyYAML is absent. CI must measure it.
- Static gates preserve the mutation-barrier order and replace obsolete subprocess/boolean-profile markers with linked DISM API, LimitAccess, serialized lifecycle, bounded streams, safe child ownership, explicit token-owner roots and empty passive roots. Root reviewed the exact assertion diff before commit.

## Recovery integration

The integrated source combines P79–P85 without replacing newer evidence readers or owner
checks. Locked Mac tests for system-repair, windows-repair-intelligence, cleaner,
pc-intelligence and maintenance-service passed: 205 tests. Integrated UI regression tests,
Svelte check (zero errors/warnings), production build, static validation, scheduler audit and
EN/AR localization passed. Full CI at the exact PR head remains the phase merge gate. The
ledger keeps the unmeasured qualification work open.

## CI gate recovery

The first integrated full CI passed its Rust and UI checks but rejected obsolete source
assertions for the moved child runner and scoped read arguments. The recursive audit now
checks fallible reader creation and both joins in each teardown branch in `process.rs`,
read-only kill guards versus retained mutating-child ownership, and DISM API LimitAccess,
cancel event and mutation-barrier order. Negative controls remove each property and remain
rejected. The four required leased read routes now include the exact request-scoped driver
and OS-peer-root cleanup forms; replacing owner, roots, scope or lease is rejected. No gate,
required count or deadline was removed. Fresh full CI at the correction head gates integration.

## Qualification limits

1. No isolated Windows VM is configured. Real SFC/cooperative console cancellation and TrustedInstaller lifetime after child exit are unverified. D19 wording is **stop at the first safe point**. The deadline requests/records stop; it cannot guarantee bounded completion if the mutating child or system call stalls. The UI retains pending state until service evidence arrives.
2. LimitAccess disables Windows Update source lookup. Windows may still consult Group Policy source locations, potentially UNC. This is not an absolute network-isolation proof; no automatic online source option or retry was added.
3. CBS markers are not a stable public verdict API. Concurrent OS servicing attribution remains conservative and may return Unknown. This implementation does not claim an authoritative OS transaction identifier.
4. Cleanup uses frozen file identity, ancestor reparse checks and an exclusive target deletion handle. It does not claim proof against every parent-directory rename/reparse race; stronger parent-chain ownership needs separate Windows qualification.
5. Crash evidence is conservatively retained in all cases until incident linkage exists. This is narrower than the eventual review-only resolved-incident category. No new destructive category or retention expansion was added under D21.
6. Skips are explained as unsafe removal and not counted as reclaimed. Existing wire data does not expose a per-file skip reason. Reclaimed bytes mean the exact removed logical file sizes, not measured physical free-space gain.
7. WinRE enabled/disabled state remains Unknown until a supported locale-independent configuration proof is established. No configuration mutation was introduced.
