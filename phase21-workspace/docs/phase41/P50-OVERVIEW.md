# P50 — rebuild the Overview on the shell's composition

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P50-OVERVIEW.md and work it end to end`
To resume after any interruption, paste the same line again and continue from §50.

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`. One session, sequential.

---

## WHY THIS SESSION EXISTS

P47 ported the design **system** into the app — tokens, the six colour roles, the
four signature elements, the bundled Arabic face, the light theme. It did not port
the **composition**. §47 recorded that honestly: the shell's sections as
components were judged "bigger than a design port".

The owner installed the result and said, correctly, that the app still looks like
the old one. Measured side by side, he is right:

**The shell's Overview** — `AetherCore.html` at the repo root — is two columns and
instrument-dense:

    SYSTEM HEALTH ORB (a large orb with an index, and four labelled channel
      rails with coloured bars and values: CPU load, Memory, Disk C:, Thermals)
      sitting beside ACTION ITEMS (cards carrying evidence chips)
    DIAGNOSTIC TELEMETRY — four stat tiles, each with a sparkline
    CORE SERVICE LOG — a monospace event log

**The app's Overview** is a single tall column of stacked cards with none of those
four sections.

Rebuild the app's Overview to the shell's composition. **This screen only.** It is
the screen the owner judges the product on, and doing one screen first lets him
see the direction before nine more are spent on it.

---

## THE CONSTRAINT THAT MAKES THIS HARD — read before writing any markup

**The shell's numbers are fabricated.** It is a mockup. `94` health index, `96`
responsiveness, `11.4 s` boot time, `0.42 ms` disk latency, `38.2k` open handles,
"214 signals", "predicts no degradation in the next 72 hours" — none of these came
from a measurement.

This product's central invariant is that it does not invent numbers. P47 already
had to make the health index traceable, and an earlier pass shipped a fabricated
`confidence 0.94` straight past a gate meant to catch exactly that.

So for **every** element you port, answer in §50 before you build it:

1. Does the app have a real source for this value? Name it — the field, the
   provider, the IPC message.
2. If yes, wire it to the real source. Never to a constant, never to a plausible
   default.
3. If no, build the element in its **honest empty state** and say so. The shell
   already has that variant — `NO BASELINE`, em-dashes on the rails, "not
   collected yet". Use it.
4. If a value exists but only sometimes, the element must distinguish
   *unmeasured* from *measured zero*. That distinction is what P42→P45 spent four
   phases building at the type level; do not throw it away at the view layer.

An element you cannot source honestly is still worth building — as an empty
channel that says what it will hold. That is the "instrument at rest" idea the
whole design rests on.

---

## THE WORK

**1 — read both sides and write the mapping first.**

Extract the shell's Overview markup (`tools/bundle_template.py` in the design
worktree round-trips the bundle; the file is also at the repo root now). Read the
app's current Overview component and everything it renders.

Produce a table in §50, one row per shell element:
`element · shell value · app's real source (or NONE) · verdict: wire / empty / drop`

Commit that table before writing any component. It is the decision record and it
is what makes this reviewable.

**Drop is a legitimate verdict.** "214 signals" and "predicts no degradation in
the next 72 hours" are marketing copy with no measurement behind them. Say so and
drop them rather than inventing a source.

**2 — the app has content the shell does not.** The current Overview carries
platform status, the protected operation, driver service, updates, a six-tile
grid, about, safety and insights. The shell was drawn for fewer, idealised
sections.

Decide for each: keep it in the new composition, move it to the screen where it
belongs, or leave it below the fold. **Do not delete a feature to make a layout
fit.** Record each decision with its reason. If a section has nowhere honest to
go, say so and leave it where it is — a slightly imperfect layout beats a lost
feature.

**3 — build it.** Derive everything from the existing token layer; do not
introduce new literals. The four signature elements stay exactly as they are:
denied-by-policy keeps its violet, its dashed 1.5px border, its 8px radius and its
exclusive icon and never shares error styling; every insight carries its evidence
chip; empty states stay honest; the policy band stays persistent.

**4 — the sparklines.** The four telemetry tiles each carry one. Check whether the
app retains enough history to draw one. If it does not, draw the tile without the
sparkline rather than with a fake one, and record what would be needed to have it.

---

## VERIFY — measure, do not assume

    node tools/verify-numbers.mjs     EXPECTED: zero untraceable numbers
    node tools/verify-arabic.mjs      EXPECTED: 7/7, zero system-font fallback
    node tools/verify-tokens.mjs      EXPECTED: every var() resolves
    tools/layout-sweep.mjs            EXPECTED: clean at 1280 / 1024 / 960,
                                      both languages, BOTH themes,
                                      populated AND with no service
    the contrast instrument           EXPECTED: nothing below 4.5:1 in either theme

The app must build, and `svelte-check` must stay at its 16-warning baseline.

**Exercise it with real data and with none.** Four responsive screenshots once
passed only because there was no data to overflow, and the light theme was broken
for months because the sweep was dark-only. Both traps are in this screen.

Screenshot the result at 1280 in both languages and both themes, populated and
empty, and put them where the owner can see them.

---

## RULES

- Every check has an **EXPECTED** value. Observed differs → stop that item, record
  the raw observation verbatim, move to the next independent item. Do not
  theorise, do not redefine the criterion.
- Explicit paths when staging. **Never `git add -A`.**
- Commit and push after each of the four steps, and move the §50 row in the same
  commit as the work.
- **This screen only.** Do not touch the other ten. If you find a defect in one,
  record it as a debt id and leave it.
- The application icon is settled — the Æ mark. Do not revisit it.

---

## REPORT

- the mapping table, every row with its verdict and its reason
- what you dropped and why, named explicitly
- what you built empty, and what data would fill it
- the raw gate output, counts not adjectives
- screenshots at 1280, both languages, both themes, populated and empty
- one paragraph: what the new Overview says that the old one did not
