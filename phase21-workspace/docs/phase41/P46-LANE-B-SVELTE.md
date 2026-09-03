# P46 Lane B — 4.D, the Svelte port

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P46-LANE-B-SVELTE.md and work it end to end`
To resume after a usage limit, paste the same line again.

**Runs on the Mac, in the DESIGN WORKTREE** at
`/Users/hasanalaaa/dev/aethercore-design`, branch `design/shell-v2`.

NOT `/Users/hasanalaaa/dev/aethercore` — another session owns `main` and
`SESSION_CONTEXT.md` §46 there and is committing continuously. Never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`.

**Do not push to main. Do not edit `SESSION_CONTEXT.md`.** Commit to
`design/shell-v2` and push there. The session that owns main will merge later.

---

## WHAT YOU ARE PORTING

`AetherCore.html` in the design worktree root — a single bundled bilingual page,
~6.3 MB, the approved design. `DESIGN.md` beside it is the system it implements.
`design/brief-v3-screenshots/` holds 13 delivered screenshots of the target.

The shell is a **specification**, not code to copy. It has zero `@media` queries
and its layout is fixed at 1280. You are building real responsive UI, not
transcribing it.

Its source is packed as a JSON string inside
`<script type="__bundler/template">`. `tools/bundle_template.py` in that worktree
extracts and repacks it, round-trip verified byte-identical. Use it to read the
source rather than fighting the bundle.

---

## NON-NEGOTIABLE — these are product invariants, not style choices

**DENIED BY POLICY never shares error styling.** Violet `#AD4EBC`, a dashed
1.5px border where everything else is solid, 8px radius where every other chip is
999px, and an icon used nowhere else. It always names the rule that refused, in
monospace. A refusal is the product keeping its promise, not a failure — this is
the single most important decision in the design system.

**Every insight carries its evidence chip**, expanding to the raw observation.
Uncitable insights are dropped before display, so the UI must have **no way** to
render one.

**Honest empty states.** `pk("Not collected yet", "لم تُجمع بعد")`. Never a fake
zero, never an invented score, never scare copy. Meters at rest show `—`, not `0`.

**Arabic is first-class and authored at source**, as `pk("English", "العربية")` at
every string. Add Arabic for every new string. **Bundle IBM Plex Sans Arabic
locally and subset it** — the product is air-gapped by construction, so a font CDN
is not merely slower, it violates an invariant.

**The six colour roles keep their meanings**: interactive · verified · healthy ·
attention · critical · denied-by-policy. `DESIGN.md` carries the hexes and the
measured colour-blind ΔE table. Interactive is solid, never a gradient — the
gradient's second stop was literally the healthy-green, and removing the gradient
removed the class of bug.

---

## THE WORK

**1 — establish the target.** Read `DESIGN.md` in full and extract the shell's
source. List every screen and every component you must produce. Commit that list
first; it is your own progress table for resuming.

**2 — the design tokens, before any component.** Port the six roles, the spacing
scale, the type scale and the surfaces into the app's token layer. Derive every
component from tokens. Do not pick values per component and reconcile later —
that is how the meanings were lost the first time.

**3 — the four signature elements**, before the screens: the denied-by-policy
state, the evidence chip, the honest empty state, and the persistent policy band.
They are the product's identity; the screens are arrangements of them.

**4 — the screens**, one at a time, committing each. Start with **Overview in its
empty state** — it is what every new user sees first and what the owner rejected
twice before approving this design.

**5 — Arabic**, verified from the rendered DOM rather than a screenshot. A
previous session judged Arabic from an image and reported a cause that the HTML
disproved.

---

## VERIFY — measure, do not assume

- render at **1280, 1024 and 960 px**. 960 is where an earlier attempt put a
  button physically over the body text. EXPECTED: no overlap, no horizontal
  scroll, no clipped text at any width, in both languages.
- Arabic: confirm from the rendered DOM that the root is RTL, that the layout is
  genuinely RTL rather than mirrored LTR, and that glyphs come from the **embedded
  face** and not a system fallback. A fallback that shapes correctly on this Mac
  will not shape on a bare Windows box.
- **exercise it with real data, not an empty app.** Four responsive screenshots
  once passed only because there was no data to overflow. Populate every screen.
- grep your own output: `denied` and `evidence` must appear. EXPECTED: zero
  percentages or scores not traceable to a measurement. A previous pass shipped
  `confidence 0.94` past this exact gate — run it properly and paste raw counts,
  not a summary.
- the app must build and run. A ported design that does not run is not a port.

---

## RULES

Every check has an **EXPECTED** value. Observed differs → stop that item, record
the raw observation verbatim, move to the next independent item. Do not theorise,
do not redefine the criterion.

Commit and push after every screen and every signature element, never in batches.

The application icon artwork is an **owner decision and is not settled**. Do not
choose one, and do not block on it.

---

## REPORT

- the screen list with each marked done or not
- screenshots at all three widths in both languages, with real data
- the raw grep counts for `denied`, `evidence`, and untraceable numbers
- how Arabic was verified, and from what
- what you could not port and why
