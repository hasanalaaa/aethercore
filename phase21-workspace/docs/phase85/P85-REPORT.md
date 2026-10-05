# P85 — supported repair and bounded cleanup

Implementation source is available; full CI, the Windows job included, passed at the integration head `3f54b51` (#108); phase acceptance remains open for the qualification gaps below. No live repair or cleanup was run on the owner's host.

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

The initial task checks ran against local source and a dirty-source Windows overlay. A separate clean Windows checkout of `855b012` passed the 1572-file workspace and seven-file workflow seals. Root-owned integration CI remains the phase acceptance authority; the follow-up results below are also explicitly source-overlay results.

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
6. Existing per-category detail now exposes deterministic counts of in-use, changed-after-preview, multiple-hardlink and other safety skips. No per-file path/error list or new wire field is claimed. Skipped approved bytes remain excluded. The displayed total is **deleted file bytes**, the exact removed logical file sizes, not measured physical free-space gain.
7. WinRE enabled/disabled state remains Unknown until a supported locale-independent configuration proof is established. No configuration mutation was introduced.


## P85-05 follow-up — explicit skip causes

The existing deletion-action detail carries four stable counts: **in use**, **changed since preview**, **multiple hard links**, and **other safety checks**. Sharing and lock violations are classified by Win32 HRESULT before aggregation; other OS errors and paths are never interpolated into the user-facing summary. Internal file-identity/metadata safety reasons identify changed evidence. No protocol, dependency, destructive eligibility or retention change was introduced.

Red-before: the native mixed deletion-action fixture returned only the generic unsafe-removal sentence; the Arabic UI test returned unavailable text. Green-after: a five-file temporary fixture deletes only the unchanged four-byte file; locked, hardlinked, changed and invalid-root files survive, each cause count is one, and sixteen approved skipped bytes are excluded. Both sharing and lock HRESULT controls pass. Native cleaner: 16 passed, one live inventory test ignored; native Clippy all targets with warnings denied passed. UI cause-count/wrapped-detail and logical-byte-label regressions passed (61/61 source UI tests); localization audit 34/34 passed. Svelte check and production build passed. Further validation is reported with the integration commit rather than inferred from this overlay.

The existing cleanup total label and verification wording now say **deleted file bytes/sizes**, in English and Arabic, rather than implying a physical free-space measurement. The existing per-item detail/result fields persist and display the aggregate cause summary; per-file skip disclosure remains outside the current wire contract.

Root integration of `af8271f` with P79–P85 preserved the P83 client-event localization checks. The integrated check passed 127 Rust tests across cleaner, maintenance-service and PC intelligence, all 75 UI tests, Svelte check (zero errors/warnings), and the production UI build. Source seals verified 1595 workspace files and seven workflow files before commit. Full final-head CI remains required after the P84 read-budget lifetime correction is integrated.

## P85-03 follow-up — cancellation throughout verification

The shared verification entry point now receives the existing owner's `RepairControl`.
Windows verification passes its flag to the existing cancellable read-only DISM, SFC and
disk checks, and checks it before each subsequent verification step. The coordinator also
checks cancellation after verification returns. That final check and the terminal commit
share the existing running-plan mutex with `cancel_repair`; the mutex is acquired only
after the actual child/API work exits. The existing mutation lease stays with the worker,
and cancelled post-mutation work records `FailedAfterMutation` with recovery required
rather than `SucceededVerified`. Wire fields, dependencies, deadlines and budgets are
unchanged; mutating children are still never force-killed.

Red-before: two actual-coordinator fixtures cancelled while a controlled first verification
call was blocked. The next verification count was one instead of zero, and an intentionally
late healthy result recorded `Completed` instead of `Failed`. Green-after: both reject the
late success; the blocked worker retains exclusive mutation ownership until actual return.
Locked macOS tests for system-repair, maintenance-service, windows-repair-intelligence and
pc-intelligence passed: 197 tests. Workspace formatting and all-target Clippy with
warnings denied passed. All eleven gate/self-test scripts and five source/localization
audits passed; optional local YAML parsing remains unmeasured without PyYAML. The source
seal verifies 1595 workspace files and seven workflow files. Native exact-commit portable
fixtures and full phase CI are pending at this source receipt; no live servicing/cancellation claim is made. The
isolated-VM and TrustedInstaller qualification limits above remain open.

Native follow-up receipt: a clean detached Windows checkout of
`1561c2547e1ca3eb98911233aa489a3518c36428`, transferred as a narrow Git bundle,
verified all 1595 workspace and seven workflow hashes. With two build jobs, both
`p85_cancel_during` actual-coordinator fixtures passed (2/2), and system-repair
all-target Clippy with warnings denied passed, compiling the real Windows
verification implementation. No live servicing or model inference was run. This
qualifies the portable cancellation regression and native build, while final
integration CI and the real-servicing limits above remain open.

### Final recovery integration and prerequisite transfer bounds

Root integration of verification cancellation passed 197 locked Rust tests using a separate build target. Reusing the intelligence lane's target had incorrectly retained its newer enum artifacts while checking the older repair tree; the independent target compiled the actual repair source and passed, without changing production code. Native verification controls and Windows all-target Clippy are recorded above at source `1561c25`.

The superseded CI at `1943619` stalled during VC++ acquisition for hours: its download handle remained open on an unchanged partial file, with an established HTTPS connection and no child build process. The run was cancelled. Source `6b9ce96` uses native Windows curl for both existing fixed Microsoft prerequisite URLs, with a 30-second connect ceiling, 120 seconds per attempt, two retries and a 360-second retry-start ceiling. This bounds transfer attempts rather than claiming a strict total including backoff or signature validation. Temporary files are admitted only after the existing valid Microsoft Authenticode checks; partial downloads are removed on curl failure. There is no new dependency, CI duration increase or signature bypass.

Four native loopback controls passed: stalled WebView2 and VC++ bodies each stop after three attempts without publishing an artifact; an initial503 followed by a complete unsigned response retries once and is rejected by the original signature checks. The fixture uses one-second test transfer ceilings and real curl/Authenticode, never downloads or installs Setup. The same fixture is now a Windows CI step before packaging. The strengthened source gate requires the fixed URL and complete transfer bounds. Final full CI must use the integration head including these corrections.

## Review 2026-10-04: the servicing drain did not merge

The two sections below describe a servicing drain from the 2026-10-04 follow-up. **It was dropped in review** (`DBT-P85-006`, commit `2fb187e`), so they describe code that is not on `main`.

- **An unbounded hold on the mutation lease.** A failed DISM cleanup waited forever on a constant, holding the machine-wide lease until the service restarted.
- **Every assessment lost its SFC result.** The drain refused SFC and DISM unless TrustedInstaller was STOPPED and then blocked in `Drop` until it stopped. The DISM scan (which starts TrustedInstaller) kept the servicing mutex meanwhile, so the SFC check after it always read unverified.

What merged from that work: the read-budget lease carried into each bounded worker, the read-only update-service checks, `wait_for_child`, and WinRE observation.

## Conservative servicing-drain source checkpoint (2026-10-04)

The prior child/API-exit boundary did not prove that TrustedInstaller had stopped. The shared
native-call drain now requires the exact SCM `SERVICE_STOPPED` observation after owned SFC
work/readers or the DISM lifecycle has returned. RUNNING, pending and unreadable states remain
unverified; they neither release admission nor certify active servicing from RUNNING alone.
The original mutation lease and DISM lifecycle lock remain held. DISM closes its session and
shuts down before its SCM drain, and releases its lifecycle lock afterward. SFC error/cancel
and read-only timeout exits drain the owned child first; a child wait error is retried rather
than treated as an exit acknowledgement. No mutating child, service or OS process is killed
by this fence. The existing read-only child stop policy and production deadlines are unchanged.

Each actual Windows assessment provider closure receives an `Arc` of its original
`ReadBudgetLease`. A timed-out or cancelled observer can return its bounded Unknown/Cancelled
state while that provider, its slot and its read admission remain charged through actual native
completion and OS drain. Late results remain discarded. A new service cannot infer idle from
restart: relevant native-call admission refuses a non-STOPPED or unreadable SCM state with the
existing declared Conflict outcome and owned EN/AR wording. Mutation drain progress is
indeterminate and describes completion as unverified; its current stage is preserved. Assessment
timeout wording now says a stop was requested and the worker may still be running.

Original behavioral RED: an actual assessment coordinator plus a controlled provider reached
its observer timeout with read budget0 instead of1. Green retains1 until actual provider return,
then releases exactly once. Shared runtime controls require explicit STOPPED, reject every other
SCM state/query absence, retain actual mutation admission through uncertain completion, refuse
another owner/workload, and ignore cancellation as a release condition. The shared function's
missing-drain controls are recorded separately from the original production RED. Local tests
for system-repair, maintenance-service and PC intelligence passed210 across17 suites; UI108,
Svelte check0 errors/warnings, build, static351, recursive120, localization and strengthened
reader/wait negative controls passed. Final all-target Clippy, source seals and full native phase
CI are recorded by the checkpoint/integration receipts rather than inferred here.

An idle RUNNING or unreadable TrustedInstaller can retain admission indefinitely. This is an
explicit conservative availability ceiling, not a bounded completion promise. Detached workers
are not joined by the service stop path; process exit destroys in-memory admission and does not
stop Windows servicing or prove it idle. Restart rechecks SCM and preserves durable incomplete
execution recovery. STOPPED is the narrow release predicate for this owned-call fence, not proof
that every possible external CBS operation is safe. The SCM observation is not an atomic Windows
reservation against unrelated system actors. No grace timeout, service-control mutation, new
wire/dependency or guessed registry CBS-idle heuristic was introduced. Native build and real SFC/
DISM/TrustedInstaller VM qualification at the new exact head remain NOT RUN in this source receipt.

### Confirmed DISM lifecycle cleanup follow-up (2026-10-04)

`DismLifecycle` now owns its `ServicingDrain` field. Its shared cleanup boundary publishes
indeterminate pending status before the owned cleanup call, and requires exact `S_OK` from
`DismCloseSession` before calling `DismShutdown`, then exact `S_OK` from shutdown. Only after
that boundary returns can field drop consult the SCM fence. An error or another success code
retains the original worker, lifecycle lock and admission indefinitely; SCM STOPPED cannot
substitute for cleanup confirmation. A failed close never proceeds to shutdown. Neither API
is blindly retried, and no service stop join or forced process termination was added.

Microsoft documents successful close as `S_OK`, with other-thread operations drained before
the session is destroyed, and requires sessions closed before the matched shutdown call:
[DismCloseSession](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism/dismclosesession-function?view=windows-11),
[DismShutdown](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism/dismshutdown-function?view=windows-11).

The actual `c3a184f` Drop body was compiled in a std-only runtime fixture with mock API
boundaries: both close and shutdown errors reproduced early guard release (exit101). The
new actual Drop body held that guard and pending notice for both failures, and released it
after ordered successful cleanup (three controls PASS). This fixture replaces the native API
and drain guard and proves destructor/control-flow behavior; it is not native DISM servicing
qualification. Shared production-helper tests additionally retain an actual MutationSupervisor
lease on close failure, shutdown failure and non-S_OK positive code, refuse the next mutation,
and prove ordered once-only cleanup and no fake pending state without owned resources.
Locked direct-dependent tests213 across17 suites, all-target Clippy, static351, recursive120
and existing gate negative controls PASS. Exact Windows build/VM qualification remains NOT RUN.
Cleanup uncertainty can cause indefinite retained admission; this availability ceiling is
intentional and cannot be described as bounded completion or a successful terminal outcome.

### Bounded WinRE configuration observation (2026-10-04)

ASTRA P85-04's newer `reagentc /info` authority replaces executable-presence inference with
a read-only query at the trusted System32 path. The WinRE ProviderSlot retains the original
assessment ReadBudgetLease inside its actual worker, accepts owner cancellation and the existing
two-minute first bound, caps decoded output through the existing child reader, discards late
results, and permits no activation/deactivation, mount, boot or recovery-image mutation.

A measured owner query returned exit0 and a complete en-US Enabled/location/BCD frame. The
strict parser recognizes only that qualified format: missing/duplicate/conflicting fields,
noncanonical/zero enabled BCD identity, nonlocal location, invalid indices/version, failed exit,
unsupported locale/encoding or an incomplete frame produce Unknown. Recognized Disabled is
Unavailable. A valid Enabled frame emits `WinReConfiguredProtectionUnverified`, with owned
EN/AR wording that explicitly leaves recovery-image usability, boot readiness and protection
unknown. It never emits `WinReAvailable`: the existing protection/fact mapping keeps WinRE
Unknown and preserves the separate prior recovery protections. No path or BCD identifier is
published in the product detail.

No existing WIM/image-information utility was found in the live crates, service or desktop source.
Image-header/metadata presence alone would not prove usable image payload or a successful
recovery boot. Accordingly no new SDK binding/dependency/framework or guessed Available flag
was introduced. Configured observation is implemented; native locked-image usability and actual
recovery boot qualification remain open. The owner's Windows UI/culture was en-US; an Arabic
product UI is localized, but an Arabic Windows console format remains unqualified and Unknown.

Local locked direct-dependent tests217 across17 suites, all-target Clippy, UI109, check0 errors/
warnings, production build, static351, recursive120, existing gate controls, freeze and EN/AR2122
parity passed. Shared tests cover the qualified frame, Unknown protection mapping, disabled and
invalid response controls; a UTF16 reader test covers code units split across pipe chunks. The
initial missing-observation control failed the positive configured/disabled tests. A mistaken
new test initially expected no protection of any kind; it was corrected to compare the prior
protection predicate, because the existing journal recovery is a separate protection. No
production protection gate was changed. Full exact native crate qualification is recorded by
the root integration receipts, not inferred from these local checks.
