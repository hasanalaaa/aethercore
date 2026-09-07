# P51 — finish the Overview: fill it, cut it, re-home the rest

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P51-OVERVIEW-FINISH.md and work it end to end`
To resume after any interruption, paste the same line again and continue from §51.

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`. One session, sequential.

---

## WHY THIS SESSION EXISTS

P50 rebuilt the top of the Overview on the shell's composition and it is right:
the orb with a derivable `51% HEADROOM` whose arithmetic is printed in its own
evidence chip, four labelled channel rails, an action-items column where every
card carries its evidence, four telemetry tiles each naming its source, and a
service log with real sequence numbers. The refusal to draw a sparkline over a
single sample — stated on the screen — is the product describing itself honestly.

Three things are wrong, and the first two are the brief's fault, not the port's.

**The screen is 4,110 px tall.** The shell's was about 1,400. The new composition
was *prepended* to the old Overview rather than replacing it, so the page starts
as an instrument and ends as a feature list. P50's brief said "do not delete a
feature to make a layout fit… if a section has nowhere honest to go, leave it
where it is", and the session followed that literally. The instruction was wrong,
not the compliance. This brief replaces it.

**The instrument is empty on first run.** The orb, all four channels and three of
the four telemetry tiles depend on the performance sampler, and the sampler only
starts from the Performance screen. So a new user opens the product and sees a
correct, honest, entirely blank instrument — and nothing tells them that visiting
another screen is what fills it. P50 raised this and correctly did not fix it,
because it changes a second screen's controller.

Read §50 before starting — its 38-row mapping table and its verdicts are the
record, and nothing there should be re-derived.

---

## ITEM 1 — make the Overview fill itself

The most prominent state of a measuring instrument must not be "at rest because
you have not found the switch".

Establish the facts first, and record them before changing anything:

- what exactly starts the sampler today, and from where (`start_perf_sampling`,
  its controller, its IPC path)
- what it costs while running — CPU, memory, IPC traffic, and any observer effect
  on the numbers it reports. §42 and §45 both treat cadence as a decision with a
  real cost, never a free default
- whether it is safe to run without an explicit user action, given that this
  product diagnoses read-only before it mutates and asks consent for everything
  it changes. **Reading telemetry is not a mutation** — but say so explicitly with
  the reasoning rather than assuming it

Then choose one, and argue for it in §51 rather than just implementing it:

  **(a) Overview starts the sampler when it opens.** Simplest, and the instrument
  is alive the moment the user sees it. Costs a background sampler the user did
  not ask for.
  **(b) Overview offers one control that starts it** — a single visible action in
  the empty state, so the blank instrument explains its own switch.
  **(c) The sampler starts with the service** and Overview simply reads it.
  Cleanest conceptually; the largest change.

EXPECTED whichever you choose: on a first run with no prior state, the Overview
either shows live channels, or shows an empty state that **names the action that
fills it**. A blank instrument with no explanation is a FAIL regardless of how
honest its em-dashes are.

Do not change the Performance screen's own behaviour beyond what this requires,
and say explicitly if you did.

---

## ITEM 2 — cut the six-tile grid

`Drivers · System Repair · Deep Clean · Startup Manager · Hardware Health ·
Crash History`

Every one of those is already a sidebar destination. The grid is a duplicate of
the navigation wearing card styling — it is not a feature, and removing it loses
nothing.

Delete it. Verify by measurement, not by reading, that each of the six is
reachable from the sidebar and that nothing in the grid did anything the sidebar
entry does not. If any tile turns out to carry a real action the sidebar lacks,
**stop, say which, and keep that one** — then report it.

---

## ITEM 3 — re-home the rest, one section at a time

Everything below the service log needs a decision that is not "below the fold".
Work them in the order they appear, and commit each:

    State Engine — current protected operation
    Driver Servicing — safe driver installation
    AetherCore updates
    Diagnostic support bundle
    About this build
    Platform capabilities
    One-click care
    Insights — what your reports mean
    Activity & Recovery — recent recovery events

For each, exactly one verdict, recorded with its reason:

  **BELONGS** — it is genuinely a whole-system summary and stays on the Overview.
    Then say what it is summarising and why the Overview is the right place.
  **MOVES** — it belongs on an existing screen. Name the screen and move it. If
    that screen already has an equivalent, merge rather than duplicate.
  **SETTINGS** — configuration, not status. About this build, update channels and
    the support bundle are the obvious candidates.
  **DROPS** — it duplicates something else. Prove the duplication.

The target: an Overview a user can read without scrolling twice.
**EXPECTED: under 2,000 px at 1280 wide, populated, in both languages.** If you
cannot reach that without dropping something real, stop at the closest honest
point and say exactly what is left and why — do not delete a feature to hit a
number.

**Do not lose a feature.** Anything you move must be reachable and working on its
new screen, verified by exercising it there — not by assuming the move worked.

---

## VERIFY — measure, do not assume

    node tools/verify-numbers.mjs   EXPECTED: zero untraceable numbers
    node tools/verify-arabic.mjs    EXPECTED: 7/7, zero system-font fallback
    node tools/verify-tokens.mjs    EXPECTED: every var() resolves
    tools/layout-sweep.mjs          EXPECTED: clean at 1280/1024/960,
                                    both languages, BOTH themes,
                                    populated AND with no service
    the contrast instrument         EXPECTED: nothing below 4.5:1, either theme

Build must pass; `svelte-check` stays at its 16-warning baseline.

Record the Overview's height at 1280 before and after, populated and empty, in
both languages. That number is this session's headline.

Screenshot every screen you moved a section **to**, not just the Overview. A move
that breaks the destination is worse than leaving it.

---

## ALSO, IF THE SESSION HAS ROOM

`DBT-P50-004` — a shared `EmptyState.svelte` inherits `flex-wrap: wrap` from
`:where(*)` in `feature-layout.css` and measures 496 px for 396 px of content. P50
found it, recorded the exact one-line fix, and correctly left it because it
renders on seven screens and P50 was scoped to one. This session touches several
screens already. Fix it and verify all seven.

`DBT-P50-002` — `Protocol v7` is a literal in the markup: true today, sourceless,
and a lie the day it changes. Wire it or drop it.

**`DBT-P50-005` needs the owner's approval before implementation — do not start
it.** Record what it needs and stop.

---

## RULES

- Every check has an **EXPECTED** value. Observed differs → stop that item, record
  the raw observation verbatim, move to the next independent item. Do not
  theorise, do not redefine the criterion.
- Explicit paths when staging. **Never `git add -A`.**
- Commit and push after every item; move the §51 row in the same commit as the work.
- The four signature elements are untouchable: denied-by-policy keeps its violet,
  dashed border, 8px radius and exclusive icon and never shares error styling;
  every insight carries its evidence chip; empty states stay honest; the policy
  band stays persistent.
- No new colour literals. Everything derives from the token layer.
- The icon is settled — the Æ mark. Do not revisit it.

---

## REPORT

- the sampler decision, with the cost you measured and the argument for it
- the Overview's height before and after, both languages, populated and empty
- the verdict table for all nine sections, each with its reason
- what moved where, and the screenshot of each destination
- anything recorded rather than worked around, with its debt id
- one paragraph: what a first-time user now sees in the first screen of the app
