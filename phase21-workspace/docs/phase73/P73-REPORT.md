# P73: the installer log moved and measured, one PR closed, three bumps taken

Three items, run from the Mac with the GitHub `windows-2025` runner standing in
for the Windows PC. The rules are unchanged: a claim without a command is a
belief, and where a measurement contradicts the brief, the measurement wins. It
won four times here:

* the brief's log prefix does not work;
* the runner cannot reproduce P71's principal split;
* Burn *does* disable logging silently when the log cannot open;
* rand 0.10 is not a drop-in bump.

---

## 0. What this host can and cannot do: changed since P72

P72 recorded that the unaccepted Xcode licence made `cc` exit 69, so nothing
that links could run. **That is no longer true.** Measured this session:
`cc --version` exits 0 (Apple clang 21.0.0), and so does `/usr/bin/git`, both
**without** `DEVELOPER_DIR`. `cargo check` and `cargo clippy` ran build scripts
to completion here (§3). The Windows-only code paths still compile only on the
runner, so every Windows verdict below comes from CI.

---

## 1. Item 1: `DBT-P49-003`. The log moved and the file DACL is clean. The row stays OPEN.

**Verdict:** the change landed and the runner measured it. The two properties
the brief conditioned ACCEPTED on did not both come back as expected:

* **Met:** no Users `WRITE_DAC`, `WRITE_OWNER` or `DELETE` on the log file's
  own DACL.
* **Not met:** "logging is not silently disabled". It is, and it is Burn's
  behaviour, not ours.
* **Not reproducible here:** the principal split.

So the row moves, but not to ACCEPTED.

### 1.1 The brief's prefix, `[ProgramData]\AetherCore\logs\AetherCore`, is wrong three ways

1. **`[ProgramData]` is not a Burn variable.** The built-in table in
   `src/burn/engine/variable.cpp` at `v6.0.2` has `CommonAppDataFolder` and no
   `ProgramData`. `FormatString` turns an unknown variable into `""` (the
   `E_NOTFOUND` branch). The prefix would therefore format to
   `\AetherCore\logs\AetherCore`. `PathSkipPastRoot` treats a single leading
   separator as rooted, so the log would land at `<current drive>\AetherCore\logs`.
2. **`C:\ProgramData\AetherCore\logs` is the product's own hardened directory.**
   `Product.wxs` creates it, the install hardener locks it to SYSTEM,
   Administrators and the service SID, and `purge-data` deletes the whole
   tree on uninstall. The unelevated bundle parent opens its log first, so on a
   real machine it would create that tree, and **own** it. Measured
   (run `35815032544`): a non-admin that created
   `C:\ProgramData\AetherCore\logs` before install still owned
   `C:\ProgramData\AetherCore` **after** the hardener ran
   (`O:S-1-5-21-…-1003 … D:PAI(…)`). It then ran `icacls … /grant` on it
   successfully, and the directory read back `aprobe:(OI)(CI)(F)`. The
   hardener resets the DACL but never the owner. That is a pre-existing product
   defect in its own right: `DBT-P73-001`.
3. **An open Burn log there breaks every uninstall.** Burn creates its log
   with `CREATE_NEW` and `FILE_SHARE_READ` only (`pathutil.cpp`,
   `PathCreateTimeBasedTempFile`), and keeps it open for the whole run.
   Measured by holding a handle the same way under
   `C:\ProgramData\AetherCore\logs` during uninstall:
   * `PurgeMachineData` failed (MSI error 1722, exit 1);
   * the uninstall returned **1603** and rolled back;
   * the service and data stayed behind.

   With the brief's prefix, that is every uninstall. `DBT-P73-002` records the
   follow-on.

### 1.2 What was applied instead

```xml
<Log Prefix="[WindowsFolder]Temp\AetherCore" Extension="log" />
```

`[WindowsFolder]` is a CSIDL variable, backslash-terminated by `ShelGetFolder`,
so this formats to `C:\Windows\Temp\AetherCore`. That is rooted, so the logging
base folder is `C:\Windows\Temp\` and the prefix is `AetherCore`. The directory
belongs to the OS, not to whoever runs the bundle first, and it is outside the
tree that `purge-data` deletes.

### 1.3 What the runner measured

The probe is `.github/workflows/windows-installer.yml`, job
`bundle-log-acl-probe`, `workflow_dispatch` only. It ran green on runs
`35812233071` and `35815032544`. Access was measured as a **real non-admin
account** (`aprobe`, created in the job), by opening each log with `CreateFileW`
for a specific right, not inferred from ACL text.

| | measured |
|---|---|
| where the logs land | `C:\Windows\Temp\AetherCore_<ts>.log`, `.elevated.log`, `_000_AetherCoreMsi.log`, for install **and** uninstall. None in `%TEMP%` |
| each log file's DACL | `O:BA`, `D:AI(A;ID;FA;;;SY)(A;ID;FA;;;BA)(A;ID;0x1200a9;;;BU)…`. Users hold read/execute only |
| non-admin `WRITE_DATA` / `WRITE_DAC` / `WRITE_OWNER` on each log | **DENIED(5)**, all three files, install and uninstall |
| non-admin `DELETE` on each log | **GRANTED**, but through the directory (see below) |
| clean install / uninstall | exit **0** / exit **0**, and `C:\ProgramData\AetherCore` is gone afterwards |

The gate asserts the first three rows. Compared with P71's
`(A;ID;FA;;;S-1-5-21-…-1001)`, which gave the user FullControl, the file-level
defect is gone.

### 1.4 The limits, stated exactly

* **The runner's `C:\Windows\Temp` is not a stock DACL.** It carries an
  explicit, non-inherited `BUILTIN\Users:(F)` (`(A;;FA;;;BU)`). That is why the
  non-admin can delete the logs (`FILE_DELETE_CHILD`), and it also holds
  `WRITE_DAC` on the directory itself. On this image, a non-admin could plant an
  inheritable ACE on `C:\Windows\Temp` and the *next* elevated log would inherit
  it. Whether a stock Windows 11 `C:\Windows\Temp` refuses that is **not
  measured**: the only machine that could answer is the Windows PC.
* **The P71 principal split is not reproduced.** The runner account is RID 500
  (`…-500`, High integrity, `EnableLUA=1`). The built-in Administrator has no
  split token, so the parent and the elevated child are the same principal.
  Starting the bundle as the non-admin `aprobe` (secondary logon) exits
  `0x80070005` and writes **no log anywhere**, even with an explicit `-log`.
  Burn stops before `LoggingOpen`, so that run says nothing about the prefix.
  So "the unelevated parent's log is created in `C:\Windows\Temp`" is
  **believed, not measured**.
* **Forced LogOpen failure: logging *is* silently disabled.** Run `35815032544`
  had the non-admin occupy every name Burn could pick for 15 minutes: 1,806
  directories named `AetherCore_<ts>.log` and `.elevated.log`. Result:
  install **exit 0**, **zero** bundle logs written. That is exactly
  `logging.cpp:186-193`: `LogDisable()`, `hr = S_OK`. The brief asked to prove
  the opposite, and it cannot be proved, because it is false. It is engine
  behaviour that bundle authoring cannot change, and it is not new: the
  `%TEMP%` default fails the same way. What the relocation changes is **who**
  can cause it. In `%TEMP%` it was only the user; in `C:\Windows\Temp` it is any
  local user, since stock Users can create folders there (believed, per the
  point above). The impact is lost support evidence, not privilege.

**Row status:** OPEN. The fix is in and the file DACL is measured. Closing it
needs two commands on the Windows PC:
1. `icacls C:\Windows\Temp`, for the stock directory DACL.
2. One unelevated install, to observe the parent's log in `C:\Windows\Temp`
   owned by the user.

The silent-disable behaviour needs an owner decision to accept.

### 1.5 Two things the probe cost

* **Every Windows job stopped building.** `adksetup.exe` 10.1.26100.2454
  failed with `0x80091007` (`CRYPT_E_HASH_VALUE`) on runs `35805995871` and
  `35806212967`.
  * **First response: a stopgap.** `0e91b3e` restored `ci.yml`'s main-scoped
    ADK cache. It held for about four hours, then the cache was evicted and
    #22's mint, #22's CI and `main`'s own CI failed at the same step.
  * **The cause.** Microsoft's ADK page lists 10.1.26100.9457 (September 2026),
    which "replaces ADK version 10.1.26100.2454".
  * **The fix.** `9acd333` pins that release's direct URL. `main`'s CI
    `35822902975` passed the ADK step on it. `DBT-P73-003`, closed.
* **Four probe defects, each found by a run:**
  * the bundle's versioned file name;
  * `net user` blocking on a Y/N prompt;
  * running the held-handle uninstall first, whose rollback left a service
    that the next uninstall could not stop (error 1921, `DBT-P73-002`);
  * `CreateFileW` implicitly requesting `SYNCHRONIZE|FILE_READ_ATTRIBUTES`,
    which made an owner's `WRITE_DAC` read DENIED. Owner rights are now
    exercised, not probed.

---

## 2. Item 2: PR #21 closed

Closed with `DBT-P70-003`'s reasoning:
* `windows-implement` emits absolute `::windows_core::` paths, which a grep for
  `use` cannot see.
* Moving only the direct declaration to 0.100.0 would put two `windows-core`
  versions in `windows-update`.

---

## 3. Item 3: three bumps, three verdicts, all adopted through the loop

| PR | verdict | why |
|---|---|---|
| #16 reqwest 0.12.28 → 0.13.4 | **adopted** | Our pin is `default-features = false, ["blocking", "native-tls"]`, so 0.13's switch to rustls as the default TLS backend does not reach us. `query`/`form` became opt-in features, and we call neither. Every method we do call is unchanged: `Client::builder`, `user_agent`, `connect_timeout`, `timeout`, `redirect(Policy::none())`, `get`, `send`, `status`, `headers`, blocking `Read`, `bytes`. `native-tls` now includes ALPN, but without `http2` it only offers http/1.1 |
| #18 rand 0.9.5 → 0.10.x | **adopted, with a source fix** | 0.10 renamed `Rng` → `RngExt`; `rand::Rng` is now `rand_core`'s base trait. Measured locally: `use rand::Rng` gives **E0599** at `idle-scheduler/src/runtime.rs:550,558`, and `use rand::RngExt` compiles. The support-bundle key is `rand::random::<[u8;32]>()`, drawn from `ThreadRng`: still ChaCha12 seeded from `SysRng`, reseeded every 64 KiB (`rngs/thread.rs` at 0.10.2). The same guarantee as before |
| #22 criterion 0.5.1 → 0.8.2 | **adopted** | Dev-only, four bench targets, and they use only `Criterion`, `criterion_group!`, `criterion_main!`, `bench_function` and `iter`. MSRV 1.86 ≤ 1.97.1. The one breaking change, async-std dropped, is unused. New transitive: `alloca` 0.4.0, a C build on Windows/Unix. CI does compile benches, through `verify-enterprise.ps1`'s `clippy --all-targets -D warnings` |

#20 (TypeScript 7) is left open, as instructed.

### 3.1 The loop, per PR

update-branch (or a local merge when GitHub refused on conflicts) → mint on the
branch → commit the three freeze records → `source_seal.py --json` read
**before** re-sealing → regenerate → CI green → squash-merge pinned to the head
SHA.

| | #16 | #18 | #22 |
|---|---|---|---|
| mint run | `35806593158` (34.9 min) | `35814290805` (35.1 min) | `35823086282` (36.1 min); the first mint, `35822464661`, died at the ADK step, see §1.5 |
| locks from mint vs branch | `Cargo.lock`, `pnpm-lock.yaml` byte-identical | byte-identical to the lock resolved locally | `Cargo.lock`, `pnpm-lock.yaml` byte-identical |
| seal `--json` before re-seal | 5 × `hash`: `Cargo.lock`, `Cargo.toml`, 3 freeze records | 6 × `hash`: the same, plus `runtime.rs` | 5 × `hash`: `Cargo.lock`, `Cargo.toml`, 3 freeze records |
| PR CI | `35809051596`, windows 1h16m | `35816683360`, windows 1h22m50s | `35825768992`, windows 1h31m55s |
| merged | `4a87a09`, tree = head tree `10f4b0e2` | `3c8c8ac`, tree = head tree `3699aa2a` | `4f9538c`, tree = head tree `d0c319bf` |

Two notes:
* **#18 conflicted** once #16 landed, on the `Cargo.toml` line pair. It was
  resolved to both bumps, and the auto-merged `Cargo.lock` passed
  `cargo metadata --locked` untouched. The runner's mint later returned it
  byte for byte.
* **A cycle is about 115 min serial**: mint about 35 min, then CI about
  80 min. P72 measured about 48 min when the mint ran in parallel with CI. The
  brief's order (mint, *then* commit the freeze) serialises them, and the
  Windows job itself now runs 76–92 min, against P72's 47–51.

### 3.2 Noticed, not changed

`crates/operation-engine/Cargo.toml` declares `rand.workspace = true`, and
nothing under `crates/operation-engine/src` names `rand`. There is no proc-macro
involved, unlike `DBT-P70-003`, so this one does look genuinely unused. It is
out of scope here.

---

## 4. Ledger

§1 goes from 71 rows / 24 open to **74 rows / 23 open** (the count rule in
`docs/LEDGER.md`; `DBT-P56-002`, `DBT-P63-010` and `DBT-P63-014` are still
malformed, unchanged).

**Moved:**
* `DBT-P49-003`: stays OPEN, with the P73 measurements.
* `DBT-P70-001`, `-002`, `-004`: CLOSED, merged.

**Opened:**
* `DBT-P73-001`: ProgramData owner squat. **Security-relevant.**
* `DBT-P73-002`: a held handle breaks uninstall, and the rolled-back service
  then fails to stop.

**Opened and closed:** `DBT-P73-003`, the ADK republish.

**What is next, by value:**
1. **`DBT-P73-001`.** A local privilege path on every machine where a non-admin
   can log on before install. The fix is in the hardener, and the runner probe
   already reproduces the defect.
2. **`DBT-P49-003`'s two Windows-PC commands.** `icacls C:\Windows\Temp`, then
   one unelevated install.
3. **`DBT-P73-002`.**
