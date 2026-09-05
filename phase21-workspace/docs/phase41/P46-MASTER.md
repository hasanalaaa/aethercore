# P46 — the master brief: audit, fix, finish

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P46-MASTER.md and work it end to end`

**To resume after a usage limit, paste exactly the same line again.** Nothing else
is needed. See "HOW TO RESUME" below.

Repo: `/Users/hasanalaaa/dev/aethercore` (Mac) or `C:\dev\aethercore` (Windows) —
each item below says which host it needs. NOT the design worktree at
`/Users/hasanalaaa/dev/aethercore-design`, and never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`.

---

## HOW TO RESUME — read this first, every session

This brief outlives any single session. A previous P41 session died at its usage
limit mid-gate, and validated work on this project has already had to be
recovered by hand once.

1. Read `phase21-workspace/docs/phase36/SESSION_CONTEXT.md` **§46**, the
   authoritative progress table. If §46 does not exist yet, you are the first
   session — create it from Part 0.
2. Find the first row not marked `DONE` or `BLOCKED-OWNER`. Start there.
3. Do not redo a `DONE` row. Do not re-derive a finding already recorded — read it.
4. **Commit and push after every single item**, never in batches. An item that is
   finished but unpushed is an item that will be lost.
5. Before you finish a session for any reason, make sure §46 reflects reality.

If a row's status and the code disagree, trust the code, fix the row, and say so.

---

## RULES THAT APPLY TO EVERY ITEM

- Every check has an **EXPECTED** value. Observed differs → stop that item, record
  the raw observation verbatim, move to the next independent item. **Do not
  theorise. Do not redefine the criterion. Do not work around it.**
- Never report a gate passed without numbers.
- Before any destructive action, write **ACTION / SNAPSHOT / EXPECTED / RECOVERY**
  and commit it first.
- **A security regression stops everything immediately.**
- Never disable Defender, UAC, Firewall or SmartScreen.
- `core.autocrlf` MUST stay `false` on both machines — this project hashes files
  and CRLF conversion breaks every SHA check. It has already caused a 1027-file
  incident.
- Windows x64 and ARM64 are both qualified (§42, §43). Any change that touches
  their behaviour must be called out explicitly with the diff.
- Fix at the type or the shared function, not per call site. One contract beats
  nine guards — that is how §42 and §45 were done, and it is the pattern here.

### The defect patterns this codebase actually produces

Check every finding against these five. They are derived from 45 phases of
evidence on this specific code, and they recur:

1. **Duplicated derivation.** The same truth computed in two places, silently
   disagreeing. `platform_tag()` returned "other" on Windows; `current_windows_sku()`
   returned Unknown on every Windows host; version had six independent deciders.
   Always ask: how many places decide this?
2. **Belief presented as measurement.** "examples aren't packaged" was believed
   twice and wrong once. Demand a measurement even when the belief is probably true.
3. **Tests that never touch the real path.** Eight unit tests passed while the
   fleet scheduler could not read back what it wrote. `crates/ipc/tests` held a
   single Unix-gated file, so a total Windows IPC failure survived to Phase 36.
   Upper-bound assertions are how all-zero readings passed for five phases.
4. **The measuring instrument itself.** A harness reporting a stale PASS. A
   verifier truncating its list to 50 while reporting 92. A build recipe exiting
   101 on a clean build. `edit_screens` returning success and writing nothing.
   Never trust a green light you have not audited.
5. **The empty-state trap.** Four responsive screenshots passed because the app
   had no data, so nothing could overflow. Exercise the real thing.

---

## PART 0 — the audit. Produce the ledger before fixing anything.

Do not start fixing. A census first is what made §45 work.

Create **§46** in `SESSION_CONTEXT.md` with a progress table covering every item
in Parts 1–4 of this brief, each row `NOT STARTED` / `IN PROGRESS` / `DONE` /
`BLOCKED-OWNER`, with an evidence column.

Then audit, and add anything you find as new rows:

**0.A — the existing debt ledger.** Read §42.9, §43, §44 and §45's tables. For
every open `DBT-*`, verify it is still real against the current code. Some may
already be fixed incidentally. Report each as `still open` / `already fixed` /
`was never real`, with file:line.

**0.B — the whole workspace builds and tests.**

    cargo build --release
    cargo test --workspace

Record every failure with its crate and raw output. EXPECTED: you will find
pre-existing failures (`DBT-P42-006`: 6 in `aethercore-driver-hub`;
`DBT-P42-007`: `offline_boundary`). Do not fix them here — classify them.

**0.C — the zero/empty census, extended beyond telemetry.** §45 pushed the
"failure becomes a confident zero" contract down to the field boundary in
`performance-telemetry`. That pattern is unlikely to live in one crate. Grep the
whole workspace for `.unwrap_or(0)`, `.unwrap_or_default()`, `return 0`, `=> 0`,
`unwrap_or(Vec::new())`, `.ok()` discarding an error, and classify each:
**A** real measured zero · **B** swallowed failure · **C** cannot tell.
Report counts per crate. Fixes come in Part 3.

**0.D — duplicated derivation sweep.** For each of these, count how many places
decide it and name them: the platform tag, the Windows SKU, the product version,
the install path, the pipe name, the service name, the model hash, capability
`native` status. EXPECTED: exactly one decider each. Any count above one is a
finding whether or not it currently disagrees.

**0.E — real-path test coverage.** For every crate with a `*_impl.rs`, state
whether any test constructs the real platform provider, or only a mock. List the
crates with zero real-path coverage. That list is the shape of the next total
failure.

Commit the ledger. **It is a deliverable on its own**, even if nothing is fixed yet.

---

## PART 1 — security review (Mac or Windows)

This project's security history is not theoretical: a review found that
`validate_targets` rejected only `".."`, so any authenticated local user could
name another user's paths and the LocalSystem service would read them and return
verbatim content. It was closed with an owner-scoped allowlist, canonicalisation
and a reparse-point refusal, with the authorization tests committed failing first.

Review the current tree for the same class of thing. This is defensive work:
verifying that requests which should be refused are in fact refused, and that
privilege boundaries hold.

Cover at least:

- every path that crosses the privilege boundary into the LocalSystem service —
  what does it accept, canonicalise, and refuse?
- the named-pipe and Unix-socket surfaces: input validation, message size limits,
  malformed-frame handling, and what an unprivileged local caller can reach
- the three crates that touch the network — `update-download`,
  `driver-acquisition`, and update-manifest-adjacent code. Update trust ships
  disabled with zero channels; verify that is still true and enforced in code,
  not just in configuration.
- signature and hash verification paths: can any be bypassed, skipped, or
  satisfied by an attacker-supplied value?
- anything that writes to a protected location, or reads a path a caller named

For each finding: the concrete failing scenario, and a regression test **committed
failing** before the fix. EXPECTED for the tree overall: no unauthenticated or
cross-user read or write is reachable. Any finding that is a real privilege
boundary break stops all other work until it is fixed.

Also re-verify, with numbers, that the air-gap invariant holds: no telemetry, no
analytics, no phone-home anywhere; the product makes zero network calls at rest.

---

## PART 2 — the open measurement questions

**2.A — DBT-P42-011, the x64 bias. This is the most interesting thing open.**

x64 Windows reads consistently high against the host's own counters: +7.47 points
and +4.09 points on cpu, and 3.47x on disk latency — three readings, same
direction. ARM64 Windows shows no such bias (§43.5: +0.85 then −0.47, opposite
signs). macOS has been measured once (§44).

Same code, different architectures, different answer. Establish why.

Do not accept "sampling noise" — §43.5 already ruled that out for x64 on the
grounds that three independent readings ran the same direction. Candidate lines of
enquiry, none privileged: the sampling window versus the host counter's window,
the denominator (logical processors, or a normalisation factor), the conversion to
basis points, and whether §45's field-boundary fix changed any of it.

EXPECTED: a named mechanism with file:line, or an explicit statement that it is
not established and what measurement would settle it.

**2.B — DBT-P42-009 and DBT-P42-010** (Windows). `perProcessorBusyBp` is never
written; gpu adapter identity and VRAM are empty because DXGI adapter traversal
was never implemented, though §41.16 3b shows the data exists via WMI. Both are
honest today — empty, not invented. Decide for each: implement, or record as
deliberately out of scope with the reason. If you implement, the observer-effect
and cadence trade-off must be stated, not silently chosen.

---

## PART 3 — fix, in this order

Work the ledger from Part 0. Priority, highest first:

1. anything from Part 1 that is a real privilege boundary break
2. every **B** from 0.C — a failure published as a confident measurement is the
   defect this whole product's credibility rests on
3. every count above one from 0.D
4. `DBT-P42-006` — diagnose the 6 `driver-hub` failures, then fix or delete them.
   Six failing tests nobody has read is either six defects or six lies.
5. `DBT-P42-007` — `offline_boundary` fails because `cargo metadata --offline`
   cannot find `android_system_properties v0.1.6` in the local registry cache.
   Recorded as environment, not code — verify that and either fix the cache or
   make the test state its own precondition instead of failing opaquely.
6. `DBT-P43-001` — §44 2.B decided `p36_relbuild.cmd` should come into the repo
   under version control rather than be fixed in place. Do that, and fix the stale
   `--example ipc_two_client_probe` reference so its exit code is trustworthy.
7. `DBT-P42-013` — `cargo test` leaves 11 files in `%TEMP%`, which confuses the
   uninstall survivor sweep. Fix the tests to clean up after themselves.
8. `DBT-P41-001` — the x64 service imports `MSVCP140`/`VCRUNTIME140`, present on
   the dev box but not in the MSI payload. On a clean machine this is a launch
   failure. Either author them into the payload or prove they are guaranteed
   present on every supported floor. Recorded as undetectable on the dev machine —
   so this needs a deliberate answer, not another observation.
9. `DBT-P40-003` — `crates/security-audit/examples/gd4_live_audit.rs`, the same
   shape as the relocated probes, macOS-only.
10. anything else in the ledger, worst-consequence first

For each: test committed failing, then the fix, then the measurement. Push after
each one.

---

## PART 4 — the remaining phases that need no owner action

**4.A — the x64 release pipeline (Windows).** Section 8 records it as never
exercised, with a `tauri.conf.json` `beforeBuildCommand` path defect. §41.4
reproduced that defect and used the existing `tauri.no-before-build.json` overlay
without modifying `tauri.conf.json`.

Exercise the pipeline end to end and fix the defect properly rather than working
around it. EXPECTED: `cargo` 0, `tauri` 0, `wix build` 0, `wix msi validate` exit 0
with **EMPTY** output (zero ICE, no suppression), payload check PASS, 16 file rows.
Record the MSI sha256 and byte size.

**4.B — Gate 5 on ARM64 (Mac, driving the VM).** §41.17 records the lifecycle as
already proven on ARM64 in an earlier session; §44 did not re-run it. Verify that
claim against evidence rather than the note. If it was proven, cite where. If it
was not, run it: build, uninstall, the fourteen-check survivor sweep, reinstall,
and re-prove every Gate 2 property.

The VM is reached with `prlctl exec "Windows 11" cmd.exe /c "..."` — the bare form
returns EMPTY. `prlctl` argv is capped near 16 KB and nested quoting through zsh
into PowerShell is fragile, so **base64 any real script on the Mac and decode it
on the VM.** The ARM64 build recipe is `p36_relbuild.cmd`: VsDevCmd arm64 +
clang-cl + Ninja + libomp, with `LIBCLANG_PATH` and `cmake` under
`C:\AetherCore-P36\toolchain`. **MSVC is not supported for ARM64 there.** MSI
packaging takes ~12 minutes and prints nothing; it is not hung.

**Resume the VM, never restore a snapshot.** These two are FORBIDDEN — they
predate the install and restoring either destroys every verified gate:

    P36-CLEAN-BASELINE       {6b721a10-8ac1-4339-9699-bd5145efda0a}
    P36-PRE-NATIVE-MUTATION  {e9434b5f-ba68-452d-ac4c-cbcc863ffb4b}

Versions 0.1.9 and 0.1.10 are recorded **must-not-ship**.

**4.C — the icon pipeline (Mac).** The final mark is an owner decision and is not
yet settled, so do not choose one. Build the pipeline that consumes it: an SVG in,
and out a multi-size `.ico`, an `.icns`, and the PNG set, wired into
`tauri.conf.json` and `Product.wxs`, replacing the committed placeholder. Prove it
with the placeholder or with `design/icon/aethercore-mark.svg` from the design
worktree, and state clearly that the artwork is pending owner sign-off.
`DBT-P36-004` stays open until the owner picks the artwork — the pipeline being
ready is not the same as the blocker being closed.

**4.D — the Svelte port (Mac).** The approved shell is
`AetherCore.html` on branch `design/shell-v2` in the design worktree — a single
bundled bilingual page with the four signature elements (denied-by-policy with its
own violet identity and dashed geometry, the evidence chip, honest empty states,
the persistent policy band), IBM Plex Sans Arabic embedded, and `DESIGN.md`
alongside it.

Port it into the real app. Non-negotiable in the port:

- Arabic stays first-class and authored at source, as it is in the shell. Bundle
  the font — never a CDN, the product is air-gapped by construction. Subset it.
- **DENIED BY POLICY must never share error styling.** A refusal is the product
  keeping its promise, not a failure. This is the most important decision in the
  design system.
- Every insight carries its evidence chip. Uncitable insights are dropped before
  display — that is a product invariant, so the UI must have no way to show one.
- Honest empty states: "not collected yet", never a fake zero, never an invented
  score, never scare copy.
- Real responsive behaviour at 1280, 1024 and **960** — 960 is where a previous
  attempt put a button physically over the body text. The shell has zero
  `@media` queries; you are adding real ones, not copying its layout.
- Exercise it with real data, not an empty app. Four responsive screenshots once
  passed only because there was no data to overflow.

Verify by grepping your own output: `denied`, `evidence`, and zero percentages or
scores not traceable to a measurement.

---

## PART 5 — the owner register. List these; do not attempt them.

Maintain a §46 section naming exactly what is blocked on the owner, what it costs,
and what it unblocks. Do not try to work around any of them, and do not treat
their absence as a gap in your own work.

- **the application icon artwork** — `DBT-P36-004`, a recorded RELEASE BLOCKER.
  The committed icon is a placeholder.
- **an Authenticode code-signing certificate.** Measured 2026-09-02 via the
  GitHub API: `actions/runners` 0, `actions/variables` 0 (so
  `AETHERCORE_CODESIGN_THUMBPRINT` is unset), `actions/runs` 0. `release.yml`
  throws if the thumbprint is absent, so the signing pipeline is scaffolding that
  fails closed for a certificate that does not exist. Achievable at roughly
  $129/year from SSL.com or Certum cloud-HSM as an individual, no hardware token.
  Azure Trusted Signing is US/Canada only. **Do not buy EV** — Microsoft removed
  its SmartScreen benefit in August 2024.
- **the UAC consent click**, at the Parallels console. Note
  `PromptOnSecureDesktop` is `0` on that VM, a non-default deviation that must
  travel with any UAC finding.
- **Gate 4, driver install and rollback.** Needs two things: the recovery media
  boot-tested (created but never booted, and §43 measured the drive detached), and
  a driver to test against — Windows Update offers this machine **zero**. Pick a
  printer-class, HID-class or USB-peripheral device; never storage, chipset or GPU.
- **Windows Server runtime qualification** — needs a Server 2025 evaluation VM.
  The installer floor is already build 17763, admitting Server 2019/2022/2025;
  domain controllers are refused by explicit policy.
- **a production update endpoint, a production key or HSM, and a dependency
  freeze from a trusted workstation.**
- **payment and distribution.** Every software Merchant-of-Record checked
  (Paddle, Lemon Squeezy, Gumroad, Stripe, FastSpring, Polar, Freemius) excludes
  Iraq for sellers, in writing. Wise blocks Iraq. Payoneer is genuinely unresolved
  and needs a live signup attempt — the single highest-leverage unknown in the
  project. Microsoft's Partner Center payout table does list Iraq, but the Store
  will not solve signing: a LocalSystem service almost certainly forces the
  MSI/EXE listing type, which Microsoft does not sign.
- Licensing architecture is already decided in `docs/adr/ADR-LICENSING.md`: an
  offline signed licence file verified locally with the existing Phase 35 Ed25519
  keyring primitives. No server, no account, and a separate key from the release
  key. **Nothing is being built yet — do not start it.**

---

## WHAT "FINISHED" MEANS

You are finished when every row in §46 is `DONE` or `BLOCKED-OWNER`, and the
report below is written. Not before, and do not stop early because a row looks
hard — record what you observed and move to the next independent row.

## REPORT — refresh this at the end of every session

- the §46 table, every row with its evidence, numbers not adjectives
- what you fixed this session and what you measured after fixing it
- every finding recorded rather than worked around, with its debt ID
- what is blocked on the owner, and what each blocker unblocks
- the single next action a fresh session should take
