# AetherCore 0.1.12 — release notes

Draft written 2026-10-06, refreshed 2026-10-10 against `main` `2a0d797a`. Every statement below cites a file in this
repository; anything the repository does not say is left out. This is the **first release, unsigned by
owner decision D32**, and it is not declared GA (see "Signing").

Two terms used throughout: a **decision** (D32, D33) is a recorded choice by the owner, in
`docs/roadmap/DECISIONS.md`; a **ledger row** (`DBT-...`) is one entry in `docs/LEDGER.md`, the project's
single list of known defects and limits, each with its status and evidence.

## What it is

A Windows maintenance and diagnostics app that works locally. After installation it needs no internet,
no account, no licence server and has no telemetry (`docs/LOCAL_ONLY.md`). It has three parts
(`README.md`, `docs/PRODUCT_CONTRACT.md`):

- a desktop app, in English and Arabic;
- the `AetherCoreMaintenance` Windows service, which does the privileged work;
- `aetherctl`, a command-line tool.

The installed acceptance probe (the automated check run on an installed copy) covers six screens of the
app: hardware, deep scan, repair, Care (the one-click care run), timeline and the assistant (`scripts/rc-provenance.py`, `SURFACE_PAGES`). On-device insights come from an embedded
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
certificate was bought or used, so Windows SmartScreen and "unknown publisher" warnings are
expected and were accepted. Every receipt says `signing: unsigned by owner decision D32` and `ga: false`
(`docs/phase87/P87-RELEASE-PROVENANCE.md`). Because the files carry no signature, check their SHA-256
before you run them: `INSTALL-UNSIGNED-0.1.12.md`.

## What is measured, and what is not

**Measured** (ledger rows named; `docs/LEDGER.md`):

- CI is green on `main`, including the Windows job and the packaging candidate.
- The embedded model's generation tests passed 67 of 67 times on the project's three Windows CI runners
  (`DBT-P62-004`).
- Seven installed acceptance runs on the owner's own Windows 11 PC (decision D33: the release is installed and
  tried on the owner's own PC, with a protected backup of its data first and a restore after) found and fixed,
  in order, `DBT-P87-013` to `DBT-P87-026`. By the ledger's own titles, one of them is a defect in the product
  itself (the desktop and the update broker carried no UAC execution level, `DBT-P87-018`); the rest are in
  the lifecycle harness, the installed-state checks and the acceptance probe. One run removed the prior
  install and deleted the owner's data, which the protected backup then restored, hash by hash
  (`DBT-P87-013`).
- **Run 7 passed** (reported by the acceptance session on 2026-10-09; the evidence files are on the owner's
  PC), as an ordinary unelevated user, on the release candidate built from source `2a0d797a`: bundle
  `AetherCoreSetup-0.1.12-x64.exe`, SHA-256
  `5425a752df800c31914d570200c79dcb788a887a3bf43c53e3564e60b07a3493`, unsigned by decision D32. It covered
  install, the installed security checks, an MSI repair that closes ACL drift, uninstall (machine data
  purged, as `release/UNINSTALL.txt` says), and an upgrade from the 0.1.11 build `578e9cd4` with the owner's
  data preserved; and all six symptoms in English and in Arabic. The promotion check answered
  `rc_eligible=true, ga=false`: eligible as a release candidate, not declared generally available.

**Not measured, or limited** (accepted, or open with the row):

| limit | what it means | row |
|---|---|---|
| No cancel after a repair has started changing things | The screen shows no cancel button after that point; a safe stop with a truthful result is deferred until an isolated Windows VM exists | `DBT-P78-002` (accepted for this release) |
| Arabic insight quality | The 1.5B model writes Arabic less faithfully than English; insights carry citations and the rule engine answers when the model does not fit | `DBT-P76-001` (accepted, revisit if seen in use) |
| Hardware matrix | Only a high-end PC (Core i7-14700K, 31.69 GiB) was measured; low-memory and mid-range PCs were **not run** | `DBT-P86-005` |
| Narrator and focus | Native focus order, Narrator and live cancellation latency were not measured on an installed build | `DBT-P86-006` |
| Real DISM/SFC stop, CBS attribution, WinRE | The code is on `main` with CI green; the real stop and attribution need an isolated VM; WinRE usability is reported as Unknown | `DBT-P85-001`..`005` |
| Unavailable provider in the repair check | On the owner's healthy PC no provider was unavailable, so that part of the check was not observed. The check passed on real progress and a declared terminal state; the probe cancels the assessment at its first progress, because a full assessment outlasts the probe's 300 s budget. Recorded as "not observed on the owner's healthy machine" and never claimed | `DBT-P87-023` (accepted by owner decision D34) |
| Driver search inside the service | The abortable Windows Update search was not measured in the installed service's own context | `DBT-P84-006` |

## The 17 open ledger rows, grouped

(`docs/LEDGER.md` §1; count by its own rule: 243 rows, 17 open, 2 malformed.)

- **Need an isolated Windows VM (6):** `DBT-P85-001`, `P85-002`, `P85-003`, `P85-004`, `P85-005`, `DBT-P84-006`.
- **Need real hardware or an installed run (5):** `DBT-P86-005`, `DBT-P86-006`, `DBT-P41-001` (the VC++
  redistributable *install* branch of the setup chain has never run), `DBT-P49-003` (installer log
  placement), `DBT-P74-002` (a low-risk install-time race, reasoned, not measured).
- **Project housekeeping, not product behaviour (5):** `DBT-P36-007` (open until a seal), `DBT-P63-009`
  (gate token comparisons), `DBT-P79-001` (CI), `DBT-P55-006` (ARM64 build recipes), `DBT-P87-027` (the
  lifecycle's evidence path is not checked for freshness).
- **The owner's own PC (1):** `DBT-P55-004` (recovery media not attached).

## Known follow-ups (test tooling, not the product)

Four small items found in review are deferred to the next release candidate, because even a test-only
change moves the source commit and this candidate is already accepted. None changes what the installed
product does (`AUDIT` review of PR #132, recorded here for the next candidate):

- the hook test does not yet pin that the locale loop returns a plain true/false;
- a failed run writes no lifecycle evidence file, so its step timestamps are lost (`DBT-P87-027` is the
  related ledger row for the evidence path);
- the probe restores the owner's saved interface language after it drains its workers, so a stuck worker
  could leave the test language set;
- the app writes its language key at start-up, so "no saved language" is restored as the start-up default,
  which looks the same.

## Uninstalling removes your AetherCore data

A full uninstall deletes **all** machine data in `C:\ProgramData\AetherCore`: the operation journal, every
recorded scan, plan and finding, logs and the local signing key. Reports and bundles you exported stay
where you saved them (`release/UNINSTALL.txt`). Back up before you remove it.
