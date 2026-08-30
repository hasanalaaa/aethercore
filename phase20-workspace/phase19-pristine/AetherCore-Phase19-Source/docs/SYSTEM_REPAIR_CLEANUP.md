# System Repair & Deep Cleanup — Phase 4

## Scope

Phase 4 adds two privileged, narrowly typed maintenance subsystems to the existing service-owned plan model:

1. **System Repair** — supported Windows integrity operations only.
2. **Deep Cleanup** — exact-file deletion from an explicit provider allowlist only.

Both subsystems use the existing immutable-plan SHA-256 digest, UAC Consent Broker, SQLite state journal, Protobuf IPC, and recovery history. The UI expresses intent through service-issued IDs; it never supplies an executable path, command line, registry expression, deletion path, or arbitrary cleanup root.

## System Repair pipeline

### Assessment (read-only)

The native service runs fixed executables from `%SystemRoot%\System32` directly with `CreateProcess` semantics through Rust `Command`; no `cmd.exe` or PowerShell is involved:

1. `dism.exe /Online /Cleanup-Image /CheckHealth`
2. `sfc.exe /verifyonly`
3. `chkdsk.exe <SystemDrive> /scan`

The assessment is service-owned evidence and receives a unique `assessment_id`. A repair plan can only be created from the current Ready assessment.

### Authorized repair

The plan always freezes the assessment ID and repair feature flags. After principal-bound, digest-bound one-shot UAC consent is atomically consumed:

1. `AwaitingAuthorization -> Preflight`
2. Freeze the fixed workflow and enter `Protected`.
3. Acquire the machine-wide AetherCore servicing/mutation lock.
4. Query Windows Update Agent servicing state (`IUpdateInstaller::IsBusy` and `RebootRequiredBeforeInstallation`).
5. Run the diagnostic-only DISM `/CheckHealth` and `/ScanHealth` checks while `mutation_started=false`.
6. Immediately before the first mutating command, commit the durable mutation barrier (`Protected -> Executing`, `mutation_started=true`).
7. Run only the selected fixed mutation workflow:
   - DISM `/RestoreHealth`
   - SFC `/scannow`
   - optional CHKDSK `/scan` remains read-only
8. `Executing -> Verifying`.
9. Re-run DISM `/CheckHealth`, SFC `/verifyonly`, and the optional CHKDSK `/scan`.
10. Mark Completed only after the verification commands finish under the accepted result policy.

CHKDSK is intentionally scan-only in Phase 4. `/f`, `/r`, `/spotfix`, forced dismount, boot-time repair scheduling, and forced restart are outside this phase.

### Command safety

- Executable locations are derived from `SystemRoot` and must be absolute existing files.
- Arguments are compile-time fixed arrays; no UI text is interpolated into a servicing argument.
- Standard input is null; stdout/stderr are captured into bounded tails.
- Commands run without a console window and have a two-hour watchdog.
- Localized console prose is not treated as the primary success authority. Exit status, durable workflow state, and post-command verification are used.
- CHKDSK documents non-zero diagnostic exit states, so `/scan` results 1–3 are surfaced as **Attention**, not silently rewritten as success or used to trigger repair switches.

## Deep Cleanup architecture

### Provider allowlist

Phase 4 has no filesystem heuristic that searches for generic “junk.” The Windows provider knows only these categories:

- Windows temporary files older than 48 hours — default selectable.
- Per-profile local temp files older than seven days — explicit review; not selected by default.
- Per-profile Direct3D shader cache older than 72 hours — explicit review; not selected by default.
- Windows Error Reporting archive/queue — explicit review; not selected by default.
- Windows minidumps and `MEMORY.DMP` — explicit review; not selected by default.

The engine does **not** enumerate or delete WinSxS, Windows Installer cache, Driver Store, Prefetch, registry entries, browser profiles/sessions, restore points, or unknown directories.

### Scan evidence

Every normal-file candidate is converted into exact immutable evidence:

- absolute discovered file path;
- approved provider root;
- final normalized provider-root identity captured from a stable directory handle;
- file size;
- last-modified timestamp;
- volume serial number;
- handle-level 128-bit file identifier;
- provider/candidate identity and scan epoch.

Candidate details returned to the UI contain counts, sizes, category descriptions, selection policy, and service-issued IDs only. Individual filenames and paths stay inside the privileged service/immutable plan.

Safety caps bound a candidate to 5,000 files and one plan to 20,000 files. Truncated candidates are explicitly marked.

### Deletion invariant

A plan never means “delete this directory.” It means “attempt these exact frozen file identities again, if they still satisfy every invariant.” Immediately before deleting each file the service:

1. Requires absolute target/root paths from the immutable service plan.
2. Verifies the target remains under the approved root.
3. Opens the provider root as a stable directory handle and requires its final path to match the root identity frozen at scan time.
4. Rejects reparse points on the approved-root ancestor chain and target path chain.
5. Opens the exact file with a native handle; locked/unavailable files are skipped.
6. Resolves the **final target path by handle** and confirms it remains below the stable approved-root handle path.
7. Re-checks file size, modification time, volume serial number, and 128-bit file identifier against frozen evidence from the same opened handle.
8. Rejects any replacement, redirection, or reparse identity mismatch.
9. Uses `SetFileInformationByHandle(FileDispositionInfo)` on that already-validated handle.

Anything changed, redirected, locked, missing, or otherwise unsafe is skipped. No wildcard deletion or recursive “delete whatever exists now” occurs during execution.

Recycle Bin emptying is deliberately deferred in Phase 4. A whole-bin API could delete items added after plan approval, which would violate the immutable exact-target invariant. It may be added only with item-level frozen identity/deletion semantics.

## Cross-subsystem serialization

Driver installation, System Repair, and Cleanup use the same machine-wide AetherCore mutation lock. Cleanup holds it for the entire deletion plan. System Repair additionally checks the Windows Update Agent servicing state before crossing its mutation barrier. AetherCore never disables Windows Update services to obtain exclusivity.

## State and recovery

Both domains persist `maintenance_executions` and `maintenance_execution_items` beside the original driver execution journal.

- A service restart during `Preflight`/`Protected` marks the plan Failed with `mutation_started=false`; no recovery action is required because no mutation was committed.
- A restart during `Executing`/`Verifying` marks the plan Failed and `recovery_required=true`.
- Neither Repair commands nor Cleanup deletion are replayed automatically after restart.
- Cleanup requires a **fresh scan** after interruption because candidate evidence is intentionally not replayed.
- Repair requires a fresh assessment/review; CBS/DISM logs and the durable operation record are surfaced as recovery evidence.

## UI contract

The Svelte UI provides:

- Repair assessment evidence, immutable plan review, UAC trigger, state/progress display, step evidence, and recovery status.
- Cleanup category scan, default/explicit selection state, total reclaimable bytes, immutable plan review, execution results, reclaimed/skipped totals, and recovery state.
- No raw file list in the Cleanup UI.
- No fake command progress. Command-level repair stages are journaled; indeterminate UI is used when Windows does not expose trustworthy fine-grained progress through this execution path.
