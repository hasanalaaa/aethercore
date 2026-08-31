# _archive-preserved

Content rescued from delivery archives before those archives were deleted in
the Phase 37 Stage 0 disk reclamation (2026-08-31).

An archive was deleted only after proving every file inside it is either
(a) byte-identical to the file at `HEAD`, or (b) present as a blob in the git
object database (so its exact bytes are reachable from history). Files that
satisfied neither test were copied here first.

| Source archive | Files not recoverable from git | Action |
|---|---|---|
| `AetherCore-Phase32-Master-Delivery.zip` | 0 of 1014 | deleted, nothing to preserve |
| `AetherCore-Phase33-Master-Delivery.zip` | 0 of 1035 | deleted, nothing to preserve |
| `AetherCore-Phase34-Master-Delivery.zip` | 0 of 1069 | deleted, nothing to preserve |
| `AetherCore-Phase35-Master-Delivery.zip` | 1 of 1124 | `phase35/` below, then deleted |

`phase35/AetherCore-0.1.0-windows-x86_64-offline.zip` (89,047 B, git blob
`d1911445e3cb2bf887b3e618ee3ecfbcf3344acf`) appeared at two paths inside the
Phase 35 archive — `release/phase35/` and
`PHASE_35_BINARY_SAFE_PATCH/BINARY_ARTIFACTS/release/phase35/` — with identical
content. It differs from the untracked copy on disk at
`phase21-workspace/release/phase35/` and was in no git object, so it is the one
genuinely unique artifact the Phase 35 archive carried.

## NOT deleted, and why

- `_archive/AetherCore-Phase32-Master-Delivery-pre-provenance-fix-956bd200.zip`
  (4.18 GB) holds 9 files absent from git, one of which is a 4.12 GB
  `PHASE_31_BINARY_SAFE_PATCH/changes.patch`. Preserving that patch separately
  would cost 4.12 GB to free 4.18 GB, so the archive is retained whole.
- `AetherCore-Phase26/27/28/29/30-Master-Delivery.zip` — no `p26`-`p30` git tag
  exists. Not proven; retained. (They are iCloud-dataless anyway and occupy
  zero local blocks.)
