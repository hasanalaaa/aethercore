# P74: the data root re-owned, the 1921 diagnosed, one unused dependency dropped

Three items, run from the Mac with the GitHub `windows-2025` runner standing in
for the Windows PC. Every Windows verdict below comes from a
`bundle-log-acl-probe` run. The measurement beat the prompt, or my own first
reading, three times:

* The 1921 is not a stop problem. It is a start that never reports.
* Fixing the hang does not make the rollback quick. MSI retries the start for
  four minutes on its own.
* `PendingFileRenameOperations` on this image does not hold the documented
  `\??\<path>` form.

| run | head | what it measured | probe |
|---|---|---|---|
| `35915825289` (A) | `94d0b29` | Item 1 fix; Item 2 diagnosis on **unchanged** purge and service code | green |
| `35921021467` (B) | `fab26fc` | Item 2 fixes; the mint for Item 3 | red: the probe's registry match (§2.2) |
| `35924932884` (C) | `d6ac286` | the raw registry dump | red: the same match, now explained |
| `35929322712` (D) | `887da87` | everything, gated | **green** |

---

## 0. Host

Unchanged from P73 §0. `cc` and `/usr/bin/git` work without `DEVELOPER_DIR`.
New this phase: `cargo clippy --target x86_64-pc-windows-msvc` works here for
crates with no C build. So the hardener was linted for Windows locally
(`-D warnings`, clean). The maintenance service was not: `libsqlite3-sys`
needs the Windows SDK headers (`stdlib.h` not found). Its Windows compile
verdict is the runner's build of runs B–D and PR CI.

---

## 1. Item 1: `DBT-P73-001`. CLOSED.

### 1.1 The decision: re-own, do not refuse

`apply()` now runs `icacls <tree> /setowner *S-1-5-18 /T /C /L /Q` on both
trees. It runs after `reject_reparse_tree`, which stays first, and before
`ensure_mutation_lock_file`, the `sc` calls and the ACL passes. So nothing
trusts the tree before its owner has been set.

* **Why not refuse a foreign-owned root.** Refusal also closes the privilege
  path. But it hands every local user a permanent denial of service: one
  `mkdir` under `%ProgramData%` and the product can neither install nor
  repair until an administrator deletes the directory by hand.
* **Why re-own.** A squatted install then ends in the same state as a clean
  one.
* **Why SYSTEM.** It is what a clean install produces: run `35809121248` read
  `NT AUTHORITY\SYSTEM` on an unsquatted data root.
* **Why `/T`.** The squatter can plant files as well as directories. The probe
  plants `logs\planted.txt` to prove the reset reaches files.
* **Why `bin_dir` too.** Users cannot create under `Program Files` on a stock
  ACL, so no squat reaches it there. The call is identical, though. The
  invariant becomes "the hardener set every owner", which needs no reasoning
  about who could have created what on a given machine.

The owner reset also comes before `/reset` for a reason. `/reset` recomputes
inherited ACEs, and `%ProgramData%`'s `CREATOR OWNER` ACE resolves to the
current owner. Re-owning first means it resolves to SYSTEM, not to the
squatter.

### 1.2 Measured and gated

The probe gates on three things. If there is no non-admin probe account, the
gate fails rather than passing vacuously.

| | run D |
|---|---|
| data root owner **before** install | `S-1-5-21-…-1003`, the non-admin `aprobe`: the squat took |
| owners of every node **after** install | `S-1-5-18 x10`, including `logs\planted.txt` |
| `aprobe`: `icacls C:\ProgramData\AetherCore /grant *<own SID>:(OI)(CI)F` | exit **5**, `Access is denied` |
| an ACE for `aprobe` in the data root DACL afterwards | none |

The same three rows read the same on runs A, B and C. The row is closed on
run D, the green run.

### 1.3 What re-owning does not cover (`DBT-P74-002`, believed, not measured)

1. **A handle opened before install.** Windows checks access when a handle is
   opened. Suppose a squatter process opens the root with `WRITE_DAC` before
   install and keeps it open through it, from a second session. It can still
   rewrite the DACL through that handle afterwards. Neither re-owning nor
   refusing revokes a handle. Only a new directory object would, and one
   opened without `FILE_SHARE_DELETE` cannot be replaced.
2. **A squatter that locks SYSTEM out.** A protected DACL granting only
   itself removes SYSTEM's `WRITE_OWNER`. Whether `icacls /setowner` enables
   `SeRestorePrivilege` in that case is not measured. If it does not, the
   install fails closed: exit 1603, a denial of service, not an escalation.

Both can be measured with two more squat variants in the same probe.

---

## 2. Item 2: `DBT-P73-002`. CLOSED, with one new row.

### 2.1 Diagnosis first, on unchanged product code (run A)

The held-handle uninstall ran as P73 ran it. Then the probe read what the
rollback had left behind:

| | measured |
|---|---|
| `sc queryex` | `START_PENDING (NOT_STOPPABLE, …)`, PID 4168 |
| process 4168 | alive |
| `sc qsidtype` | `NONE`. The hardener sets `unrestricted` |
| `sc qc` | `AUTO_START`. The hardener sets delayed-auto |
| `sc stop` | **1052** ("the requested control is not valid"), in 14 ms |
| `service.jsonl` | `ERROR service failed: maintenance service token policy mismatch`, 25 ms after the rollback started it |
| MSI log | rollback `ServiceInstall(…)`, then `ServiceControl(Action=1)` from 21:07:23.947 to 21:11:29.572: **4 min 05 s**, then `Info 1920` |

The chain, each link measured:
1. The rollback's `ServiceInstall` re-creates the service from the MSI row.
   The hardener, which set the SID type, is not in the rollback script.
2. `verify_maintenance_service_token` refuses a token without the service SID.
   That check is correct, and it stays.
3. `service_main_impl` returned from that failure **before registering a
   control handler or setting any status**. So SCM never heard anything: the
   service sat at `START_PENDING` with its process alive inside
   `StartServiceCtrlDispatcher`.
4. MSI's start waits 4 minutes and gives 1920. Every later stop is refused,
   because the service accepts no controls. The next uninstall's
   `StopServices` waits and gives the 1921 P73 saw.

### 2.2 Two fixes, and what run D measured

* **The service always reports `SERVICE_STOPPED`.** `windows_service_host.rs`
  registers the control handler first. It then runs the token check and the
  server in `verify_then_run`, and reports `STOPPED` on every exit, with
  Win32 exit code 1 on failure. The check still runs before `SERVICE_RUNNING`.
* **purge-data tolerates an undeletable file.** Decided yes, because a leftover
  log is worth less than a failed uninstall that rolls back into a broken
  service. The reparse refusal and the first `remove_dir_all` are unchanged.
  After one 1.5 s pause, `delete_or_schedule` walks the tree:
  * it deletes what it can;
  * it schedules the rest with `MoveFileExW(path, NULL,
    MOVEFILE_DELAY_UNTIL_REBOOT)`, children before their directory, because
    the boot-time pass runs in order and a directory goes only once empty;
  * only a path that cannot even be scheduled fails the action.

  This is what Windows Installer does with its own files in use. It uses a
  local `extern "system"` declaration, the pattern `consent-broker` already
  uses, so there is no new dependency and no lock change.

Run D:

| | measured |
|---|---|
| uninstall with a `FILE_SHARE_READ`-only handle held on `logs\AetherCore_held.log` | exit **0**, service gone |
| left under the data root | 3: the held file, `logs`, the root |
| `PendingFileRenameOperations` | `*1\??\…\logs\AetherCore_held.log`, `*1\??\…\logs`, `*1\??\C:\ProgramData\AetherCore`, each followed by an empty destination. All 3 scheduled, in that order |
| rollback forced with a junction under the data root | uninstall 1603. The restored service reads **`STOPPED`**, Win32 exit 1, process gone, and `sc stop` gives 1062 at once |
| the next uninstall | `StopServices` 2 ms, whole uninstall 2 s, exit 0 |

**The `*1` prefix.** Runs B and C went red on the probe, not the product.
Run B's gate matched `\??\<path>` and found none of the three. purge-data
can only exit 0 if `MoveFileExW` succeeded, so run C dumped every
`PendingFileRename*` value raw. The source strings read `*1\??\<path>`, which
is not the documented format. The gate now strips an optional `*<n>` prefix.
What `*1` means to the boot-time pass is **not known**. The OS wrote it
through `MoveFileExW` for its own consumer.

**Not measured:** the deletion at reboot. A reboot ends the runner job.

### 2.3 What is still wrong: `DBT-P74-001`

A rollback still takes **251–257 s** (runs B, C, D). The restored service
cannot start: the rollback recreated it with SID type `NONE`. With the fix it
now stops cleanly every time. The System log shows `7023` "Incorrect
function" about every 36 s, seven times. MSI keeps retrying the start anyway,
and the product is left registered with a service that will not run until a
repair re-runs the hardener. That is a defect in its own right, and a larger
change than this item.

Fix direction: a rollback custom action running `apply`, sequenced between
`StopServices` and `DeleteServices`. In reverse it then runs after the
service is re-created and before it is started. The alternative is
`MsiServiceConfig` authoring, which `Product.wxs` deliberately avoided. Before
choosing it, measure whether that table is honoured on rollback. Opened as
`DBT-P74-001`.

---

## 3. Item 3: `rand` out of `operation-engine`, through the loop

**No macro path, checked by command.**
* `grep -rn rand crates/operation-engine` hits only `Cargo.toml`.
* The crate has no platform `cfg`, so the Mac build covers every path.
* Its macros are the serde and thiserror derives, which emit no `rand` paths.
* With the dependency removed, `cargo check --all-targets`, `cargo clippy
  --all-targets -D warnings` and `cargo test` (5 passed) all succeed. A macro
  expanding to `::rand` would have failed there. That compile is the check
  `DBT-P70-003`'s grep lacked.

**The lock changed.** `Cargo.lock` loses exactly one line, `"rand"`, from this
crate's dependency list. So it went through the loop, not a plain commit:

1. Commit (`fab26fc`).
2. Mint on the branch: run B's `build-unsigned-candidate`. `Cargo.lock` and
   `pnpm-lock.yaml` are **byte-identical** to the branch.
3. Commit the three freeze records (`ee4cf5f`). They differ from `main` only
   in the hashes of `Cargo.lock` (`2eb5ccb1…`) and
   `crates/operation-engine/Cargo.toml` (`f54e8e68…`), both re-derived here
   with `shasum`, plus the two baselines covering them.
4. `source_seal.py --json` before re-sealing: exactly those three files,
   reason `hash`.
5. Re-seal.

Every commit on the branch was sealed the same way: `--json` read first, and
the failures matched the files touched.

---

## 4. Ledger

§1 goes from 74 rows / 23 open to **76 rows / 23 open**, counted with the
rule in `docs/LEDGER.md`. The same three rows are malformed as before:
`DBT-P56-002`, `DBT-P63-010`, `DBT-P63-014`.

**Closed:**
* `DBT-P73-001`, run D.
* `DBT-P73-002`, run D.

**Opened:**
* `DBT-P74-001`: a rolled-back uninstall restores a service that cannot start.
* `DBT-P74-002`: the two residuals of re-owning, believed and not measured.

**What is next, by value:**
1. **`DBT-P74-001`.** Any rollback leaves a broken service, and the probe
   already forces one deterministically.
2. **`DBT-P49-003`'s two Windows-PC commands.** Unchanged from P73.
3. **`DBT-P74-002`.** Two more squat variants in the same probe.
