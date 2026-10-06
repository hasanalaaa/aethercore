# AetherCore 0.1.12 — release notes

Draft written 2026-10-06 against `main` `582c4105`. Every statement below cites a file in this
repository; anything the repository does not say is left out. This is the **first release, unsigned by
owner decision D32**, and it is not declared GA (see "Signing").

## What it is

A Windows maintenance and diagnostics app that works locally. After installation it needs no internet,
no account, no licence server and has no telemetry (`docs/LOCAL_ONLY.md`). It has three parts
(`README.md`, `docs/PRODUCT_CONTRACT.md`):

- a desktop app, in English and Arabic;
- the `AetherCoreMaintenance` Windows service, which does the privileged work;
- `aetherctl`, a command-line tool.

The installed acceptance probe covers six surfaces: hardware, deep scan, repair, Care, timeline and
the assistant (`scripts/rc-provenance.py`, `SURFACE_PAGES`). On-device insights come from an embedded
Qwen2.5-1.5B model, with a rule-based fallback when the model does not fit its time budget (`README.md`;
`DBT-P62-004`).

What it will and will not do to your PC (`docs/PRODUCT_CONTRACT.md`):

- **Repair** runs read-only checks first; its only changing steps are DISM `/RestoreHealth` and SFC
  `/scannow`.
- **Drivers** are installed only for a driver you select, with your one-time UAC consent. A verified
  System Restore point and an export of the current driver are required first.
- **Updates** inside the app are off by default (`update-trust.json` ships with `enabled: false` and no
  channels, `docs/LOCAL_ONLY.md`). The only network paths are user-started: update download and driver
  download.

## Signing: unsigned, and what you will see

The first release is **unsigned** (owner decision D32, `docs/roadmap/DECISIONS.md`). No code-signing
certificate was bought or used, so Windows SmartScreen and "unknown publisher" warnings are expected and
were accepted. Every receipt says `signing: unsigned by owner decision D32` and `ga: false`
(`docs/phase87/P87-RELEASE-PROVENANCE.md`). Because the files carry no signature, check their SHA-256
before you run them: `INSTALL-UNSIGNED-0.1.12.md`.

## What is measured, and what is not

**Measured** (ledger rows named; `docs/LEDGER.md`):

- CI is green on `main`, including the Windows job and the packaging candidate.
- The embedded model's generation tests passed 67 of 67 times on the project's three Windows CI runners
  (`DBT-P62-004`).
- The first two installed acceptance runs on the owner's own Windows 11 PC (owner decision D33) each found
  real defects, which are fixed on `main`: `DBT-P87-013` to `DBT-P87-018`. Among them, one run removed the
  prior install and deleted the owner's data, which the protected backup then restored, hash by hash;
  the run now restores it in every owner-host run (`DBT-P87-013`).
- The result of the final installed acceptance run was not available when this was drafted, so it is not
  stated here.

**Not measured, or limited** (accepted, or open with the row):

| limit | what it means | row |
|---|---|---|
| No cancel after a repair has started changing things | The screen shows no cancel button after that point; a safe stop with a truthful result is deferred until an isolated Windows VM exists | `DBT-P78-002` (accepted for this release) |
| Arabic insight quality | The 1.5B model writes Arabic less faithfully than English; insights carry citations and the rule engine answers when the model does not fit | `DBT-P76-001` (accepted, revisit if seen in use) |
| Hardware matrix | Only a high-end PC (Core i7-14700K, 31.69 GiB) was measured; low-memory and mid-range PCs were **not run** | `DBT-P86-005` |
| Narrator and focus | Native focus order, Narrator and live cancellation latency were not measured on an installed build | `DBT-P86-006` |
| Real DISM/SFC stop, CBS attribution, WinRE | The code is on `main` with CI green; the real stop and attribution need an isolated VM; WinRE usability is reported as Unknown | `DBT-P85-001`..`005` |
| Driver search inside the service | The abortable Windows Update search was not measured in the installed service's own context | `DBT-P84-006` |

## The 16 open ledger rows, grouped

(`docs/LEDGER.md` §1; count by its own rule: 242 rows, 16 open, 2 malformed.)

- **Need an isolated Windows VM (6):** `DBT-P85-001`, `P85-002`, `P85-003`, `P85-004`, `P85-005`, `DBT-P84-006`.
- **Need real hardware or an installed run (5):** `DBT-P86-005`, `DBT-P86-006`, `DBT-P41-001` (the VC++
  redistributable *install* branch of the setup chain has never run), `DBT-P49-003` (installer log
  placement), `DBT-P74-002` (a low-risk install-time race, reasoned, not measured).
- **Project housekeeping, not product behaviour (4):** `DBT-P36-007` (open until a seal), `DBT-P63-009`
  (gate token comparisons), `DBT-P79-001` (CI), `DBT-P55-006` (ARM64 build recipes).
- **The owner's own PC (1):** `DBT-P55-004` (recovery media not attached).

## Uninstalling removes your AetherCore data

A full uninstall deletes **all** machine data in `C:\ProgramData\AetherCore`: the operation journal, every
recorded scan, plan and finding, logs and the local signing key. Reports and bundles you exported stay
where you saved them (`release/UNINSTALL.txt`). Back up before you remove it.
