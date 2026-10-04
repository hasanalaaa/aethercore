# P84-04 export capacity and protected ancestors

Base: `3ecacc3b4015ebb9452d35f2cdd72bdf1f3ec7ad`, branch `codex/p84-export-preflight`.
Initial capacity/readback checkpoint `299412dc1edbe0fed592459148c44a32785102f6` and
READ-only sharing checkpoint `d9b56c8e883a7f95c8e34bc423ca5fa46a618d04` are historical
steps, **not qualified protection**: real FSCTL controls disproved directory sharing alone.
ASTRA-PLAN P84-04 requires measured capacity and non-reparse protected local roots; D16
allows no protection bypass. No wire, dependency version, safety-tier or consent change.

## Production flow and protection

`WindowsInstallPlatform::backup_driver` → public `export_driver_package` → `export_into`
validates the observed OEM INF, acquires a root lease, admits only its owned sentinel,
measures capacity, executes the existing trusted System32 PnPUtil command, acquires a real
exported-file witness, releases sentinels, seals, then returns evidence. The coordinator
still re-reads that evidence before any install. No command is started for a capacity deficit.

`SetupGetInfDriverStoreLocationW` resolves the existing Driver Store package read-only.
A metadata-only walk rejects reparse/unknown entries, empty packages, overflow and more than
4096 entries. Sizes are rounded to the destination allocation unit (`GetDiskFreeSpaceW`),
with one MiB allowance; `GetDiskFreeSpaceExW` measures caller/quota-available bytes. Failure
or unknown measurements block export. This snapshot is not a reservation or an exact proof
of all filesystem overhead; later export/seal/re-read errors still block install.

A held no-DELETE child name keeps a directory nonempty. Windows refuses an in-place
reparse tag on a nonempty directory. Every existing path component is opened without
following its final reparse point and kept open without DELETE sharing. A missing component
is created only after its parent has a held existing witness or an owned sentinel. Existing
ancestors are read-only; if no bounded trusted witness is available, creation is refused.
New empty directories and the admitted empty export leaf receive an owned `FILE_CREATE`
sentinel with `FILE_DELETE_ON_CLOSE`, retained throughout export. Normal child/sibling writes
remain available: ancestor handles share READ and WRITE. All opens/creates of these children
use one `NtCreateFile` helper, a single relative component, retained `RootDirectory`,
`OBJ_DONT_REPARSE` and `FILE_OPEN_REPARSE_POINT`. Thus the empty-directory interval before
its first witness cannot redirect a path-based write. Handle metadata is rechecked before
returning the lease to path-based callers.

After export, a real file (or a retained ordinary directory chain ending in a real file)
is opened with READ_DATA and no DELETE sharing **before** any sentinel is dropped.
Truncation may change contents but cannot remove this held name or make its ancestors empty.
There is no empty handoff gap and no permanent manifest exception. Sentinels are removed by
OS handle lifetime on success/error; exported-file leases remain through sealing. The traversal
and retained candidate budget is 4096 per lease, with at most 128 path components. Fixed local
disks only; UNC/mapped/removable roots and observed reparse components fail closed.

Manifest/member readback opens deny writer/delete sharing and reject reparse metadata.
Manifest writing uses `create_new`, refusing to overwrite an injected pre-existing manifest
or link. Readback losing a trusted member remains the existing typed `Changed` outcome.
Unsafe code is confined to the native module; each successful native handle transfers
immediately to `File` RAII. No manual raw-handle lifetime or guessed NTSTATUS conversion.

Two features of the existing `windows = 0.62.2` dependency were necessary:
`Wdk_Foundation` (`OBJECT_ATTRIBUTES`) and `Wdk_Storage_FileSystem` (`NtCreateFile`). Win32
`CreateFileW`/standard OpenOptions do not accept the retained RootDirectory required for
relative creation. No new dependency/version/lock change. The root reviewed this addition;
only Cargo.toml's manifest hash and the baseline digest in freeze metadata changed.

## Red evidence and corrections

- Shared actual export flow called the export callback with required=2/available=1 before
  the capacity guard. It now refuses it; equal capacity writes real temporary INF/CAT files,
  seals and verifies. Actual footprint controls cover 14 bytes, cluster rounding and unknown/empty.
- Actual `seal_export` reached through an ancestor symlink originally succeeded. It now fails
  closed, as does verification through the substituted ancestor.
- Native 299412d initially passed 9/10: mklink fixture setup failed. Its test-only writer
  overlay then proved READ|WRITE sharing admitted a writable directory handle (exit 101).
- d9 exact initially passed 10/12: two mklink fixture setups failed. Isolated execution
  reproduced the failure, disproving parallel-lock speculation. The fixture's executable
  path contained `System32/cmd.exe`; normal native segments plus ordinary drive spelling for
  cmd fixed it. Production does not canonicalize away junctions.
- Actual FSCTL_SET_REPARSE_POINT using an OS-created junction buffer subsequently succeeded
  through a metadata handle while d9's READ-only directory guard was held (exit 101).
  This is the critical counterexample; d9 is not secure.
- First anchored-witness diagnostic passed 10/13. A metadata-only file witness did not prevent
  remove_file; it was changed to READ_DATA. A disappeared final subdirectory witness surfaced
  InvalidRoot instead of existing Changed; readback maps this drift to Changed. The raced-tag
  positive control uses actual writable access; metadata-only positive returned ACCESS_DENIED.
- Second diagnostic passed 13/14: all tag mutations were refused, but a metadata-only handle
  returned ACCESS_DENIED rather than the speculative expected DIR_NOT_EMPTY. The assertion
  still requires every mutation to fail; full writable access separately requires actual
  DIR_NOT_EMPTY. Third diagnostic passed **14/14**, default parallel runner, no serial-test workaround.

Native controls include: unguarded empty-tag positive; held nonempty full-writer negative
`0x80070091` (ERROR_DIR_NOT_EMPTY); READ_ATTRIBUTES negative `0x80070005`; WRITE_ATTRIBUTES
and GENERIC_WRITE negatives `0x80070091`; ancestor/leaf rename refusal; ancestor junction
refusal before creating an escaped child; first-witness tag race with no target write;
actual sibling mklink and child-file writes while guards are held; continuous handoff,
no persisted sentinel, refused witness removal, and zero-byte truncation preserving
DIR_NOT_EMPTY after handoff. Empty unowned ancestors are not populated; failure drops owned
sentinels. Manifest collision preserves the original file. No actual PnPUtil call is made.

## Verification receipt

Mac affected checks: backup 11 + install unit 6 + coordinator 7 = **24 passed**, zero failed/ignored.
Affected-package all-target Clippy `-D warnings` and Windows GNU all-target Clippy passed.
Static checks: 351 passed; dependency freeze hash/tool check passed; formatting and
CRLF-aware diff check passed. Source seal: 1630 workspace + 7 GitHub files passed.
Raw logs: `/private/tmp/p84-export-witness-mac-final.log`,
`/private/tmp/p84-export-witness-clippy-mac-final.log`,
`/private/tmp/p84-export-witness-clippy-cross-final.log`,
`/private/tmp/p84-export-witness-static-final.log`.

Final exact sealed native receipt is pending the code checkpoint and fresh archive extraction.
Diagnostic overlays are explicitly unqualified. Third diagnostic raw log:
`/private/tmp/p84-export-witness-native-diagnostic3.log` (14 passed, no failed/ignored).
Earlier failures: `/private/tmp/p84-export-tag-write-red.log`,
`/private/tmp/p84-export-nonempty-fsctl-diagnostic.log`,
`/private/tmp/p84-export-witness-native-diagnostic.log`,
`/private/tmp/p84-export-witness-native-diagnostic2.log`.

Native fixture host: Windows 11 Pro 10.0.26200 build 26200; PowerShell 5.1.26100.9444;
Rust/Cargo 1.97.1. Isolated root `C:\dev\codex-p84-fixtures`, own `target`, jobs=2.
Only backup library fixtures and affected-package Clippy run; ignored live-export test not run.
Root CI106 ran concurrently in the other approved slot. This is correctness evidence,
not an isolated performance measurement.

## Remaining qualification limits

Namespace leases protect active operations; no persisted namespace identity is claimed after
release. Unreadable/empty unowned ancestors, a witness budget/depth ceiling, collisions, quota
or allocation failures are honest fail-closed availability limits. Source package content and
free space may change after the capacity snapshot. The exact native controls qualify temporary
filesystem mechanics on this one host only. No real PnPUtil export compatibility measurement,
installed driver change/recovery, restore point, Driver Store mutation, product/service mutation,
destructive VM scenario or physical disk-full test was performed. Full integrated phase CI and
any installed/VM qualification remain root-owned. No push was performed.

References: [NtCreateFile](https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntcreatefile),
[OBJECT_ATTRIBUTES](https://learn.microsoft.com/en-us/windows/win32/api/ntdef/ns-ntdef-_object_attributes),
[nonempty-directory reparse rule](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/4aeefef8-92c3-4abc-af7a-a610caf8a165),
[SetupGetInfDriverStoreLocationW](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupgetinfdriverstorelocationw),
[GetDiskFreeSpaceExW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdiskfreespaceexw),
[GetDiskFreeSpaceW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdiskfreespacew).
