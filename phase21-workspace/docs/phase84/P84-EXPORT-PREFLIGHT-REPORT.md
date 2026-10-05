# P84-04 follow-up: export capacity and protected ancestors

A follow-up to P84-04 (`DBT-P84-005`), started on 2026-10-04 on a branch built on `3ecacc3`. It
was then reviewed and reduced before merge (`DBT-P84-009`). This file records what merged; the
branch history keeps what was dropped.

## What the export does now

`WindowsInstallPlatform::backup_driver` → `export_driver_package` → `export_into`:

1. **The INF.** The observed OEM INF name is validated (`oemN.inf`).
2. **The path.** `guard_root(destination, create)` checks every directory from the volume root down to the destination. Missing directories are created; a link or junction anywhere refuses the export; so does anything that is not a directory. The export folder must be empty.
3. **The capacity preflight.** `SetupGetInfDriverStoreLocationW` resolves the package already in the Driver Store, read-only. A metadata-only walk refuses links, unknown entries, an empty package, size overflow and more than 4096 entries; it rounds each file to the destination's allocation unit (`GetDiskFreeSpaceW`) and adds 1 MiB. `GetDiskFreeSpaceExW` gives the bytes available to the caller. A shortfall, or a measurement that fails, stops the export before PnPUtil starts. This is a snapshot, not a reservation: a later write, seal or re-read error still stops the install.
4. **The export.** The existing System32 `pnputil.exe /export-driver` runs.
5. **The seal.** `seal_export` checks the path again, reads every file through a handle that refuses writers and reparse points, and writes the manifest with `create_new`, so a file already there is never overwritten.
6. **The re-read.** The coordinator still re-reads the export against the sealed manifest (`verify_export`) immediately before the install.

## Why the checks are path-based

The backup root is `%ProgramData%\AetherCore\recovery\driver-backups`. The install hardener gives
that tree to SYSTEM, Administrators and the service account only: Users get no access at all
(`apps/install-hardener/src/main.rs`). So nothing less privileged than the service can create,
rename or re-tag anything there between a check and its use.

The follow-up also built an NT-native scheme for that race: `NtCreateFile` opens relative to held
directory handles, delete-on-close sentinel files, and pinning one child of every directory so
none can be re-tagged as a reparse point. It was dropped for three reasons:

- **It defends nothing new.** It protects against a writer the DACL already excludes.
- **It needed an owner decision.** It added the `Wdk_Foundation` and `Wdk_Storage_FileSystem` features to the `windows` dependency and re-baselined the dependency freeze, which H6 leaves to the owner.
- **It reached outside the product.** To create a folder it pinned the first existing child of each ancestor without delete-sharing. Under `C:\ProgramData` that is another program's folder, held so it could not be deleted or renamed during an export. A child it failed to open was skipped silently.

## Proof

- The driver-backup unit tests run on macOS and on Windows:
  - capacity shortfall stops the command;
  - real fixture files export and seal;
  - footprint sizes, with an empty package refused;
  - an existing manifest is not overwritten;
  - a link in an ancestor, or the export replaced by a link, is refused (Unix).
- The coordinator's protection tests are unchanged.
- The live `pnputil` export probe (`tests/live_backup.rs`, run with `--ignored`) runs on the PC.
