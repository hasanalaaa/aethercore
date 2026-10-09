# P87 installed acceptance source

This producer exercises the actual installed Tauri/WebView2 frontend and service;
it never upgrades source fixtures into installed qualification. The lifecycle
requires a dedicated disposable Windows 11 machine, or the owner's own PC under
owner decision D33. It also requires the exact RC inventory: signed, or unsigned
under D32. It needs an ordinary interactive explorer-owner token and an immutable
previous RC for the upgrade step.

## Evidence and ownership

`scripts/p87-installed-acceptance.ps1` binds source, bundle, locale, SID and Care
UUID. The six symptom cases require actual hardware/cleanup text, performance
fault labels, Care reconnect and same-run service-restart persistence, repair
progress/terminal/unavailable-provider observations and an actual completed
cleanup scan with the owned no-op explanation. Missing conditions remain blocked.

Hardware, DeepScan, Repair, Care, Timeline and assistant each require separate
runtime JSON, accessibility-tree JSON and PNG witnesses. Runtime evidence binds
actual installed selector, route, locale, connected service and nonempty text.
The verifier requires typed witness roles, hashes, populated AX nodes and PNG
signature bytes. Reusing Care captures as Timeline or assistant fails. Capturing
the assistant starts no model question; DeepScan navigation starts no scan.

A bounded loop waits for real locale/backend readiness after reload. Scan
witnesses wait for the exact completed scan ID in the rendered DOM. The producer
uses native named arguments and a shared deadline. Only its own assessment may be
cancelled. Known read workers and Care must reach terminal states before ownership
is released; unknown transport/worker state preserves the installed service.
It closes only its own desktop. A timed-out helper is observed, never force-killed.

The disposable cleanup fixture is exclusively created and recorded with exact
path, length, SHA256 and timestamps. Cleanup requires released ownership and
unchanged bytes; a changed file is preserved. Restart verification accepts only
the same typed, ordinary-user, closed-desktop pending receipt, creates no fixture
and updates the selected Care case by identity even when cases are reordered.

## Verification and limits

The native actual-function suite exercises invocation arguments, terminal and
foreign-worker controls, deadline/desktop failure, receipt pins and booleans,
rendered scan identity, byte-preserving fixture cleanup, reordered Care receipts
and isolated native process argument/exit/timeout behavior. Its controllers and
copied cmd.exe fixture invoke no installed product or service. Portable verifier
controls additionally reject incomplete/blocked receipts, repair progress omissions
and missing/substituted surface roles. Native Authenticode rejection remains a
separate Windows check.

The GitHub workflow runs producer SelfTest, actual-function controls, lifecycle
controllers and the RC verifier suite before normal native qualification. Source
seals cover the scripts and workflow. Fixture success is source evidence only;
signed installed EN/AR acceptance, Narrator, 200% scale and wider hardware claims
require their actual observations. The production-signing protections/certificate,
dedicated disposable VM and previous signed RC were absent in the recorded
inventory; GA and signed installed qualification remain blocked.

The parent scheduled shell forwards the probe's actual `$LASTEXITCODE`; its
previous unconditional `exit 0` could mask a pending/failed receipt and prevent
the required restart branch. An exit-only temporary script exercises the actual
encoded command in a child PowerShell, reproduces the former mask, and checks
both failure and success propagation. These native controls are pending final
CI, rather than counted as already executed. This follows Microsoft's documented
[script exit semantics](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_automatic_variables?view=powershell-7.5).

An expanded portable audit failed P17-STATIC-017 because the two pre-existing
P27 qualification records lacked expected evidence, severity and executable
hooks. Six fields were filled from Phase27 architecture and actual host-local
tests; both records stay open and require the original multi-host matrix. No
audit assertion or qualification status was relaxed.

The same audit's obsolete literal permission marker and all-finding description
key assumption were corrected to follow the actual typed producer-to-normalizer
path and the remediation action admission match. Existing gate controls now
reject removed/misclassified permission mappings, removed producer serialization,
missing admitted description translations and unknown generator shapes. The
full audit, including actual Rust and Svelte checks, subsequently passed; native
qualification remains explicitly separate. The portable RC suite ran40
cases:39 passed and the native-only Authenticode case was explicitly skipped. Its actual
source selector check rejects the former nonexistent Timeline class. Final
Windows CI executes the updated73 producer assertions and native observer exit
controls; earlier69-assertion native evidence is not relabelled as this new head.

First integrated native CI37123653317 at d94d984 executed the prior69 producer
controls successfully, then rejected the new Case5 fixture. The fixture's generic
`$state` controller collided with the actual Wait-Terminal snapshot variable via
PowerShell dynamic scope. The controller was renamed and diagnostic case/events
retained; production and assertions were unchanged. That run remains failed.
The next exact-head native run must prove the updated fixture and the observer
exit controls before merge.

## Owner host under D33 (2026-10-04)

`phase16-installer-lifecycle.ps1 -OwnerHostAccepted` replaces the disposable-machine
acknowledgement (passing both is rejected). It refuses to start without the D33 row. The
producer receives `-OwnerHostAccepted` and records `host: owner-host-d33`. Restart
verification also rejects a receipt from another host. Before anything is installed, the run:

1. Stops the owner's service, so the database is copied quiescent.
2. Creates `%ProgramData%\AetherCore-owner-backup-<UTC>` with a protected DACL
   (`D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)`, no inherited Users grant).
3. Copies `%ProgramData%\AetherCore` into it and checks every file's SHA256 against the source.
   It then writes `MANIFEST.sha256`, whose hash is bound into the lifecycle evidence.
4. Removes the prior install through its own cached Burn bundle. If there is no bundle, it uses
   the MSI product code; more than one install is refused.
5. Applies the unchanged clean-host guards.

The normal lifecycle then runs unchanged. At the end it reinstalls the accepted RC and observes
the service `Running` within 30 seconds (`owner-host-service-restored`). If any step fails, the
`finally` block does the same restore instead of an uninstall, unless a test worker still owns
the service. To restore the data by hand, stop the service, copy `ProgramData-AetherCore` from
the backup back to `%ProgramData%\AetherCore`, and start the service.

Without an upgrade baseline the lifecycle may run acceptance and record its observations. The
`signed-upgrade-preserves-owner-data` step is then absent, so `rc-provenance.py` cannot promote
that evidence. No older-version RC exists before the first release: main and the installed
product are both 0.1.11.

The native fixture runs the actual new functions with substituted cmdlets and a real robocopy
over temporary directories. Its controls cover:
- the decision rows;
- D32 rejecting `Valid` and `HashMismatch`, and signed mode rejecting `NotSigned`;
- the owner-host task switch and same-run restart;
- the backup copy, manifest binding, protected DACL and stopped service;
- bundle and MSI prior uninstall, refusal of an ambiguous install, and rejection of a leftover service;
- reinstall and `Running` observation.

It failed on the parent (`Assert-OwnerDecision` missing) and passes on the PC.

## First real D33 run and its fixes (2026-10-05)

The first owner-host run of the 0.1.12 D32 RC stopped at `installed-security-boundaries`. It found three defects that no fixture or earlier run had reached:
- **DBT-P87-014.** The service-token verifier decoded every SID as ANSI garbage.
- **DBT-P87-015.** The lifecycle expected data to survive a full uninstall.
- **DBT-P87-013.** Removing the prior install purged the owner's data, and the restore only reinstalled the product.

The backup is what kept the data. It was restored by hand and verified against its manifest (6 of 6 files), and the service reopened it. The owner-host restore now does this in every run, including after a failed step. The lifecycle evidence records `owner_data_restored`, and `rc-provenance` refuses owner-host evidence without it.

Lane 3 reviewed this (`AUDIT/REVIEW-125.md`): approved with no P1, and the three P2s are fixed as listed in `DBT-P87-013`. One limit is open (P3-b). The restore mirrors the backup with `/MIR`, so files that a newer installer creates and the older backup lacks are removed. That worked for 0.1.11 to 0.1.12. A later version with new state files may need `install-hardener apply` or an MSI repair after the restore.

## D33 run 5 (2026-10-09)

Run 5 is the first run in which the owner-host lifecycle reached the six installed symptoms: the 0.1.12 D32 RC at `0ca0d233`, run as the owner, unelevated.

In English, four symptoms passed on real WebView evidence:
- hardware owned text
- performance provider labels
- cleanup owned text
- the Care no-op explanation

The other two stopped on probe defects, not product defects:
- **DBT-P87-021.** The Care run id format.
- **DBT-P87-022.** The typed `repair.stateUnavailable` for a user with no assessment yet.

Arabic and the upgrade from 0.1.11 had not started. The lifecycle kept the owner's data in the backup, as designed; it was restored by hand and matches the manifest.

## D33 run 6 and owner decision D34 (2026-10-09)

Run 6 (RC at `febcc01b`) passed five of the six symptoms in English:
- hardware, performance, cleanup and the Care no-op;
- Care persistence: the same run `care-1791564077214` survived an independent reconnect and a service restart.

Repair observed real progress and a terminal state, but no unavailable provider: the owner PC is healthy, and a full assessment outlasts the probe budget. Under D34 (`DECISIONS.md`), on the owner host only, that check reads "not observed on the owner's healthy machine, by owner decision D34". It never reads passed, and acceptance and promotion list it under `not_observed`. The path itself is covered on Windows in CI by the system-repair `bounded.rs` and `winre.rs` tests and the probe's Case 4 fixture named in DBT-P87-023. The lifecycle restored the owner's data automatically. Arabic and the upgrade had not run yet.

