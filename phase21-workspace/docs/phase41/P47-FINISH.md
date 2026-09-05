# P47 — the last brief: land what exists, finish what remains

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P47-FINISH.md and work it end to end`

**To resume after a usage limit or an interruption, paste the same line again.**
Read §47 in `SESSION_CONTEXT.md`, find the first item not `DONE` or
`BLOCKED-OWNER`, continue there. Nothing else is needed from the owner.

Primary host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Item 5 needs the
physical Windows PC and is explicitly gated on that machine being reachable —
if it is not, record it and finish everything else. Never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`.

**One session, working sequentially.** Do not open parallel lanes and do not
delegate commits or pushes to sub-agents. The previous phase ran three lanes and
paid for it: a sub-fork exceeded its scope on a peer's say-so, an uncommitted
change was swept into an unrelated commit, and a lane started in the wrong
directory. Sequential is faster here than parallel was.

---

## RULES

- Every check has an **EXPECTED** value. Observed differs → stop that item,
  record the raw observation verbatim, move to the next independent item. Do not
  theorise, do not redefine the criterion, do not work around it.
- Never report an item done without numbers.
- **Use explicit paths when staging. Never `git add -A`.** Run `git status`
  before staging, not after.
- Commit and push after every item, never in batches.
- A security regression stops everything immediately.
- Never disable Defender, UAC, Firewall or SmartScreen.
- `core.autocrlf` stays `false` — this project hashes files.
- Fix at the type or the shared function, never per call site.

Read `SESSION_CONTEXT.md` §42, §44, §45 and §46 before starting. Create **§47**
as your progress table, with a row per item below.

### The five patterns this codebase produces

1. **Duplicated derivation** — the same truth decided in two places.
2. **Belief presented as measurement** — Arabic was believed to render in the
   bundled font for an entire port; `CSS.getPlatformFontsForNode` showed 1,445 of
   1,795 characters were painted by Tahoma. Measure the thing itself.
3. **Tests that never touch the real path** — upper-bound assertions let all-zero
   readings pass for five phases.
4. **The measuring instrument itself** — a recipe exiting 101 on a clean build; a
   gate reporting zero untraceable numbers while `confidence 0.94` sat in the file.
5. **The empty-state trap** — screenshots pass when there is no data to overflow.

---

## ITEM 1 — land the Svelte port. Do this first; it is the work most at risk.

Branch `design/shell-v2` in the worktree at `/Users/hasanalaaa/dev/aethercore-design`
carries a complete port pass: **34 commits ahead of main, 142 files, +16,531 lines**,
including the four signature elements, the design tokens, and two permanent
verification tools (`verify-arabic.mjs`, `verify-numbers.mjs`).

**It is also 156 commits BEHIND main**, because main absorbed all of P46 while the
port was being written. Sitting on a stale branch is where work gets lost. §46's
own table still reads "4.D Svelte port — NOT STARTED", because the session that
owns main cannot see another branch. Close that gap.

**1.A — merge main into the branch first**, in the design worktree, and resolve
there. Never resolve a 156-commit divergence on main.

The two sides are mostly disjoint — main's P46 work is in `crates/` and
`services/`, the port is in `apps/ui/` — but check specifically for conflicts
where they meet: the `has_*` wire fields added in P46 (`5c1406d`) are consumed by
the UI, and `product-identity` became the single decider for the service and
product names (`a042a8c`, `71fc629`), so any name literal the port introduced must
now read from it.

**1.B — prove the port still works after the merge**, before proposing it to main:

    the app builds                     EXPECTED: 0 errors
    node tools/verify-arabic.mjs       EXPECTED: 7/7, zero system-font fallback
    node tools/verify-numbers.mjs      EXPECTED: zero untraceable numbers
    tools/layout-sweep.mjs             EXPECTED: clean at 1280/1024/960, both languages,
                                       populated AND with no service

Re-run the Arabic check from the rendered DOM, not from CSS. The port's headline
finding was that the CSS looked correct while the engine painted Tahoma.

**1.C — merge to main** once 1.B is green. Then finish what the port left open,
recorded in `P46-LANE-B-PROGRESS.md`:

- the screens that got no dedicated pass — Performance, Hardware, Clean, Repair,
  Recovery, Crash, Fleet, Command Palette, Consent, Sidebar. They carry the token
  layer and measure clean, but were never checked against the shell's section
  vocabulary.
- the real defects the port surfaced and did not all fix: the About panel never
  requesting its data (`void load;` — a reference, not a call), four dead buttons
  in the Insights panel, seven undefined custom properties, and a fixture using
  `'UserSelectable'`, a value the service never emits, which meant the driver
  refusal path was never displayed.
- ~430 literal colour values remaining in `feature-layout.css`. Decide: migrate
  them to roles, or record explicitly why they stay. Do not leave it unstated.

One measurement disagreed with spec and was recorded honestly: the denial chip's
border reads 1px where 1.5px is declared, because Chrome rounds border widths.
The approved shell renders identically, so it matches the baseline. Leave it;
do not chase it.

---

## ITEM 2 — Gate 5 on ARM64

§41.17 says the lifecycle was proven on ARM64 in an earlier session. §46 records
4.B as NOT STARTED. **Verify the claim against evidence before re-running
anything** — if it was proven, cite where and mark it DONE. "A note says so" is
not evidence; this project has been wrong that way twice.

If it was not proven, run it: build, uninstall, the fourteen-check survivor sweep,
reinstall, and re-prove every Gate 2 property. EXPECTED: zero survivors on all
fourteen; any survivor is a finding recorded with its exact path, never deleted by
hand and called a pass.

The pipe DACL must be byte-identical to:

    O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)

`(A;;0x12008b;;;AU)` is the SAME DACL — `FR|DC = 0x120089|0x2 = 0x12008b`. **Not
drift.** Read it with:

    [System.IO.File]::Open('\\.\pipe\AetherCore.Maintenance.v7',
      [IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite)

then `.GetAccessControl().Sddl`. `engineLabel` must report `localModel` against
the **running service** — `aetherctl self-check --load-model` returning exit 7 is
correct by design and is not the proof.

VM operating facts: `prlctl exec "Windows 11" cmd.exe /c "..."` — the bare form
returns EMPTY. argv caps near 16 KB and quoting through zsh into PowerShell is
fragile, so **base64 any real script on the Mac and decode it on the VM**. The
ARM64 recipe is `p36_relbuild.cmd` (VsDevCmd arm64 + clang-cl + Ninja + libomp;
`LIBCLANG_PATH` and `cmake` under `C:\AetherCore-P36\toolchain`). **MSVC is not
supported for ARM64 there.** MSI packaging takes ~12 minutes and prints nothing —
it is not hung.

**Resume the VM; never restore a snapshot.** Enumerate before you act:

    prlctl snapshot-list "Windows 11" -j

As of 2026-09-03 that returns 8 snapshots, oldest `P36-VM-QUALIFIED {a38386fa}`,
current `P40-PRE-HOUSEKEEPING {d652cd40}`. Earlier briefs named two "forbidden"
pre-install snapshots that **are not present on the VM** — that was an inherited
claim, never measured, and §46.15 records the correction. The rule that replaces
it: resume only, take a NEW named snapshot before anything destructive, never
delete or reuse an existing one. Versions 0.1.9 and 0.1.10 are must-not-ship.

---

## ITEM 3 — the icon pipeline

The artwork is an owner decision and is **not settled**. Do not choose one and do
not block on it. Build the pipeline that will consume it.

SVG in → a multi-size `.ico`, an `.icns`, and the PNG set out, wired into
`tauri.conf.json` and `Product.wxs`, replacing the committed placeholder. Prove it
end to end with `design/icon/aethercore-mark.svg` from the design worktree as a
stand-in, and state plainly in the commit that the artwork is provisional.

`DBT-P36-004` stays **open** until the owner picks the artwork. A ready pipeline
is not a closed blocker — do not mark it closed.

Note for the record: that stand-in mark, as currently used in the app, still
carries a sky gradient outside the six roles. Report it; do not change it.

---

## ITEM 4 — the 2.B decision

`DBT-P42-009`: `perProcessorBusyBp` is never written on Windows.
`DBT-P42-010`: GPU adapter identity and VRAM are empty because DXGI adapter
traversal was never implemented, though §41.16 3b shows the data exists via WMI.

Both are honest today — empty, not invented. Decide each: implement, or record as
deliberately out of scope with the reason. If you implement, state the
observer-effect and cadence trade-off explicitly rather than choosing it silently.

---

## ITEM 5 — the x64 items. Gated on the machine being reachable.

The physical Windows PC has been offline. **Check reachability first and record
the result.** If it is not reachable, mark every sub-item `BLOCKED-MACHINE`, say
so plainly, and finish everything else — do not wait on it.

**5.A — independently verify the 4.A claim.** §46.16 records a peer session
reporting the x64 release pipeline ran end to end with zero ICE, and a real
`Product.wxs` defect found and fixed. That is recorded as **their claim, not a
verdict**. Re-run it yourself: `cargo` → `tauri` → `wix build` → `wix msi validate`
→ payload check. EXPECTED: every exit code 0, `wix msi validate` output **EMPTY**,
payload PASS, 16 file rows. Record the MSI sha256 and byte size.

**5.B — DBT-P42-011, the x64 bias.** Recorded: +7.47 and +4.09 points on cpu,
3.47x on disk latency, three readings all the same direction; ARM64 shows no such
bias. **Those numbers predate P45's field-boundary fix.** Re-measure on the
current build before explaining anything. If the bias no longer reproduces, that
is the finding — say so and stop. Do not hunt a cause for something that is gone.
If it does reproduce, establish the mechanism with `file:line`, or state it is not
established and name the measurement that would settle it. **Do not fix it** — a
cadence or denominator change alters every reading on the qualified platform.

**5.C — DBT-P41-001.** The x64 service imports `MSVCP140` and `VCRUNTIME140`,
present on the dev box but not in the MSI payload. On a clean machine that is a
launch failure. Either author them into the payload, or prove they are guaranteed
present on every supported floor (build 17763 up, Server SKUs included) with a
citation. EXPECTED: a decision with evidence, not another observation.

---

## THE OWNER REGISTER — list, do not attempt, do not route around

Keep this current in §47. These are the only things left that no agent can do:

- **the icon artwork** — `DBT-P36-004`, a recorded RELEASE BLOCKER
- **an Authenticode certificate** — measured absent (0 runners, 0 variables,
  0 workflow runs); ~$129/year, SSL.com or Certum cloud-HSM, individual, no
  hardware token. **Not EV** — Microsoft removed its SmartScreen benefit in
  August 2024
- **Gate 4** — boot-test the recovery media (created, never booted, and measured
  detached), and nominate a printer/HID/USB-class device; Windows Update offers
  this machine zero drivers. Never storage, chipset or GPU
- **the UAC consent click**, at the Parallels console — `PromptOnSecureDesktop`
  is `0` there, a non-default deviation that must travel with any UAC finding
- **Windows Server runtime qualification** — needs a Server 2025 evaluation VM
- **a production update endpoint, a production key or HSM, and a dependency
  freeze from a trusted workstation**
- **payment** — every MoR checked excludes Iraq for sellers in writing; Payoneer
  is unresolved and needs a live signup attempt

---

## WHAT FINISHED MEANS

Every §47 row is `DONE`, `BLOCKED-OWNER`, or `BLOCKED-MACHINE`. Not before.

## REPORT — refresh at the end of every session

- the §47 table, evidence on every row, numbers not adjectives
- what landed, and what you measured after it landed
- anything recorded rather than worked around, with its debt ID
- the owner register, and what each blocker unblocks
- the single next action, in one sentence
