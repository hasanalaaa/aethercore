# P55 — destructive action record, Item 4.C

Written and committed **before** the action, per the rule this repository keeps
because the machine has no snapshots and System Restore is the only rollback.

Date: 2026-09-12. Machine: the physical Windows PC at `C:\dev\aethercore`,
elevated (`IsInRole(544)` = `True`).

---

## ACTION

1. Uninstall the AetherCore currently on this machine — **`0.1.11`**, ARP key
   `{0F9F349D-01C8-B3C2-7242-83B5D29047C9}`, service `AetherCoreMaintenance`
   **Running**, installed at `C:\Program Files\AetherCore`. This is P49's
   artefact and predates phases 50–54, including `ba9973b`'s fix for ephemeral
   insight state crossing authenticated principals.
2. Install `AetherCoreSetup-0.1.11-x64.exe` from the green CI run
   `34683812129` — 1,121,407,585 B, sha256
   `747bb6ee3265c4b141bfc58277c4fea71bc8155826b54503f3b7562556b6de3f`.
3. Photograph every screen it shows (Item 4.D).
4. Uninstall it **through the bundle**, and sweep for survivors.

**Then, separately — not part of 4.C's measurement:**

5. Once the survivor sweep is recorded and committed, install the bundle once
   more, so the machine ends carrying the fixed build rather than nothing.

   Owner instruction 2026-09-12, and the reasoning is recorded because it
   changes what "done" means here: the build this machine started with predates
   `ba9973b`, so it is running a version where ephemeral insight state crosses
   authenticated principals. Leaving that installed is not an option; leaving
   nothing installed is a worse end state than leaving the fixed build.

   This install is measured and reported **on its own**, after 4.C's numbers are
   committed, so it cannot contaminate them.

   **It is a fresh install, not an upgrade.** Step 4 leaves zero survivors, so
   step 5 lands on a clean machine. Upgrade-in-place (`QD-035-001`) is
   deliberately **not** exercised here: if it is worth qualifying it gets its own
   item, its own restore point and its own EXPECTED values, rather than
   happening as a side effect nobody planned to interpret.

Owner decision 2026-09-12 authorised this, having been told it removes the
existing installation. The brief's own EXPECTED end state for 4.C is the
product uninstalled; step 5 is a deliberate addition on top of that.

## SNAPSHOT

* A named System Restore point, created immediately before step 1 and
  **verified by enumeration** — `Checkpoint-Computer` returning without error is
  not proof; `Get-ComputerRestorePoint` listing the new point by description is.
  Baseline before the action: **5** restore points.
* The system image on `D:` — version `09/02/2026-09:46`, bare-metal capable,
  10 days old. Not re-created; see `../LEDGER.md` §2 and `DBT-P55-003`.
* The pre-mutation state of the installed product, recorded to JSON before
  anything is removed: ARP entry, install directory listing, service
  configuration, and the fourteen survivor locations.

## EXPECTED

| step | EXPECTED |
|---|---|
| restore point | `Get-ComputerRestorePoint` lists a point whose description matches, count 5 → 6 |
| uninstall of P49's build | ARP entry gone, `C:\Program Files\AetherCore` gone, service absent |
| install of the CI bundle | exit 0; ARP entry present at `0.1.11`; service present; install dir populated |
| installer UI | title bar reads **"AetherCore Setup"**, not "AetherCore Setup Setup"; the Æ mark, not WiX's placeholder; progress names **"AetherCore"** rather than sitting on "Initializing..." |
| uninstall through the bundle | **zero survivors on all fourteen** |
| reboot | none requested. Any reboot request is recorded, not accepted |

## RECOVERY

Failure modes, worst first, and the way back from each:

1. **Uninstall leaves the machine without AetherCore and the new install
   fails.** Consequence: no AetherCore installed. This is *not* a recovery
   emergency — the brief's end state for 4.C is uninstalled anyway, and the
   1.1 GB artefact is on local disk at
   `out/ci-artifact/…/AetherCoreSetup-0.1.11-x64.exe` and re-downloadable from
   run `34683812129`. Recovery: re-run the installer.
2. **The install corrupts the service or leaves a half-state.** Recovery: the
   restore point from SNAPSHOT, which is what
   `APPLICATION_INSTALL` restore points exist for. Verified present before the
   action begins, so it is known to exist rather than assumed.
3. **The machine will not boot.** Recovery: the `D:` system image, via recovery
   media. **This path is currently unproven** — the media is not attached
   (`DBT-P55-004`) and has never been boot-tested. This is why Gate 4, which
   *can* produce this failure mode, is not being started.

   An MSI install/uninstall is not in the class of action that produces an
   unbootable machine: it writes to `C:\Program Files`, one service, and
   registry keys under the product's own hive. It touches no driver, no boot
   configuration, no storage stack, and no firmware. P49 performed exactly this
   install and uninstall on exactly this machine and the machine survived it.
   That is the basis for proceeding while recovery is unproven — and it is also
   why the same reasoning is explicitly **refused** for Gate 4.

## NOT DONE

* No driver is installed. Gate 4 does not start.
* Defender, UAC, Firewall and SmartScreen are not touched.
* `D:` is not written to and is not formatted.
* No reboot is accepted.
