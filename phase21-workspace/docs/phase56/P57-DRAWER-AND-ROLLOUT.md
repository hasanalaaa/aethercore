# P57 — the assistant drawer, then the whole product

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase56/P57-DRAWER-AND-ROLLOUT.md and work it end to end`
To resume after any interruption, paste the same line again and continue from the
P56/P57 rows in `phase21-workspace/docs/LEDGER.md`.

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`. One session, sequential.

Read `docs/phase56/DIRECTION.md` and `docs/phase56/DENSITY.md` before anything.
They are the decisions; this brief is the execution.

**The owner has approved the direction.** The redesigned Overview measured:
15 type sizes → 6 (all tokens), 6 neutral surfaces → 3, 63 prose words → 0
populated, 167 → 24 empty, 1,715 px → 1,578 px. That is the bar for every screen.

Load `frontend-design`, `impeccable`, `ui-ux-pro-max` and `design-critic` and use
them. Load `dataviz` before touching any meter, gauge or chart.

---

## ITEM 1 — the assistant drawer. Build this FIRST.

The engine works: the embedded model generates (it never had, before P56 —
`DBT-P56-002`), it grounds every claim in collected evidence, and it refuses
questions it cannot ground. `2.B`, `2.C` and `2.E` are closed.

**There is still nothing to type into.** The owner saw the new Overview and his
first question was where the assistant is. That is the gap to close before
anything else.

Build it exactly as `DIRECTION.md` §"Where it lives" specifies:

- **an overlay drawer on the inline-end edge, 26rem wide.** It overlays, never
  reflows — no screen changes layout because the assistant is open.
- available on **every** screen
- `Ctrl+/` opens and focuses the input · `Enter` sends · `Escape` cancels a
  streaming turn and then closes · `Tab` reaches every citation chip
- also openable from the rail
- below the `58rem` container breakpoint it becomes a full-width sheet, like every
  other overlay in the shell
- **streaming and cancellable.** Cancellation must actually stop generation, not
  just hide the output — `2.E` has the test.
- **every response carries its evidence chips**, and a response that cannot be
  grounded renders as the honest refusal, never as prose. This is the invariant
  the whole feature exists under: the product drops any insight it cannot cite.
- a model failure surfaces a **declared fault with a reason**, never an empty
  answer and never a silent fallback. `engineLabel` keeps reporting `localModel`.

**The empty state counts evidence, not capabilities.** As decided:

    maintenanceHistory 8 · timelinePattern 12

It tells the user what it can answer *today*, in rows it actually has. If nothing
has been scanned, it says so and names the scan that would change that.

Arabic first-class, authored at source, verified **from the rendered DOM** — never
from a screenshot. A previous session judged Arabic from an image and reported a
cause the HTML disproved while 1,445 of 1,795 characters were painted by Tahoma.

---

## ITEM 2 — settle the numeral convention, once, everywhere

Arabic currently renders readings in Arabic-Indic numerals (`٥١٪`, `١٢٬٤٨٠`)
while the service log stays Western (`seq 1`, `12:30:00`). That is two
conventions in one screen.

Pick one for technical readings across the whole product. State the reason in
`DIRECTION.md` and apply it everywhere, **including the log**.

The argument to weigh, not a decision handed to you: this is an instrument, its
readings are scanned rather than read, and much of its numeric content sits in
monospace beside Latin identifiers, paths and hashes that cannot change. Whatever
you choose, a number and the identifier beside it must not fight each other.

---

## ITEM 3 — the rest of the product, two screens per commit

Apply the direction to every remaining screen:

    Deep Scan · Performance · Hardware Health · Crash History · Drivers
    System Repair · Deep Clean · Startup Manager · Activity & Recovery
    Fleet · Settings

For each pair, report: type sizes before/after, neutral surfaces before/after,
prose words before/after **populated and empty**, words per reading, and height.
The instrument is `apps/ui/tools/measure-density.mjs` and it reads the rendered
DOM, not the source.

**A warning that will save you from a wrong conclusion:** `layout-sweep` defaulted
to `pages: ['overview']`, so **eleven of these screens have never been swept in
this project's history**. P56 found a real regression only because it swept all
twelve — Arabic ascenders clipped by 2px on every screen, 72 of 144.

Expect findings in these screens that are not yours. **Record them as
pre-existing, with evidence that they predate this phase**, and fix them only
where the fix is in scope. Do not fold someone else's defect into your own
measurement.

**If a screen's content cannot survive the copy cut without losing a fact, keep
the fact and say so.** Delete explanation, never information.

---

## WHAT MUST NOT CHANGE

- **DENIED BY POLICY** keeps its violet identity, dashed 1.5px border, 8px radius
  and exclusive icon, and **never shares error styling**. A refusal is the product
  keeping its promise.
- Every insight carries its evidence chip.
- Honest empty states. A meter at rest shows `—`, never `0`.
- The persistent policy band.
- The six colour roles keep their meanings. Interactive is solid, never a gradient.
- **No number appears that cannot be traced to a measurement.**
- The icon is settled — the Æ mark.

---

## VERIFY — every item, not just at the end

    node tools/verify-numbers.mjs   EXPECTED: zero untraceable
    node tools/verify-arabic.mjs    EXPECTED: 7/7, zero system-font fallback
    node tools/verify-tokens.mjs    EXPECTED: every var() resolves
    tools/layout-sweep.mjs          EXPECTED: clean across ALL twelve screens,
                                    both languages, BOTH themes, populated AND empty
    the contrast instrument         EXPECTED: nothing below 4.5:1, either theme

Build passes; `svelte-check` stays at zero errors and zero warnings — P56 reached
that, so it is now the baseline, not an aspiration.

Exercise every screen with real data and with none.

---

## RULES

- Every check has an **EXPECTED** value. Observed differs → stop that item, record
  the raw observation verbatim, move to the next independent item.
- Explicit paths when staging. **Never `git add -A`.**
- Commit and push after every item; move the LEDGER row in the same commit.
- End every commit with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- Do not report a compile clean by reading `cargo check | tail -3` — P56 did that,
  saw warnings, called it clean, and had committed a service that did not build.
  Check the exit code.
- No new colour literals. Everything derives from the token layer.

---

## REPORT

- the drawer: screenshots open and closed, both languages, both themes, mid-stream
  and after a refusal
- the numeral decision and its reason
- the before/after table for all twelve screens
- what you found that was pre-existing, with the evidence that it predates you
- the raw gate output, counts not adjectives
- one paragraph: what a user can now ask their machine that they could not before
