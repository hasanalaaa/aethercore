# P56 — the redesign, and the local AI surface

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase56/P56-REDESIGN-AND-CHAT.md and work it end to end`
To resume, paste the same line again and continue from the P56 rows in
`phase21-workspace/docs/LEDGER.md`.

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`. One session, sequential.

**Load these skills before you design anything**, and actually use them rather
than acknowledging them: `frontend-design`, `impeccable`, `ui-ux-pro-max`,
`design-critic`. Load `dataviz` before touching any meter, gauge or chart.

---

## THE OWNER'S TWO COMPLAINTS, IN HIS WORDS

**"Everything has far too much text."** He is right, and it is measurable. Today
every card carries a label, a title, a subtitle and a descriptive paragraph, and
every sidebar entry carries a description under it. The interface explains itself
continuously instead of showing the machine.

**"There is no way to talk to the local AI."** Also right, and worse than he
thinks — see Part 2.

He wants premium, modern, restrained, at the level of the best software
companies, and he explicitly likes the orb. Keep that instinct: the orb is the one
element that shows rather than tells.

---

## WHAT MUST NOT CHANGE — these are product guarantees, not style

- **DENIED BY POLICY** keeps its own violet identity, its dashed 1.5px border, its
  8px radius and its exclusive icon, and **never shares error styling**. A refusal
  is the product keeping its promise, not a failure.
- **Every insight carries its evidence chip**, expandable to the raw observation.
- **Honest empty states.** `pk("Not collected yet", "لم تُجمع بعد")`. Never a fake
  zero, never an invented score, never scare copy. A meter at rest shows `—`.
- **The persistent policy band.**
- **Arabic is first-class and authored at source**, `pk("EN", "AR")` at every
  string, with the font bundled locally — the product is air-gapped, so a CDN is
  not merely slow, it breaks an invariant.
- The six colour roles keep their meanings. Interactive is solid, never a gradient.
- **No number appears that cannot be traced to a measurement.** `verify-numbers.mjs`
  exists because this was violated twice.

---

## PART 1 — the design direction, decided and proven on one screen

**1.A — measure the problem before solving it.** Count, per screen: words of
prose, distinct type sizes, distinct surface elevations, and the ratio of
explanatory text to data. Publish the table. "Too much text" becomes a number you
can move.

**1.B — write the direction as a short document**, three to five principles with
a sentence each, in `docs/phase56/DIRECTION.md`. It must answer:

- **What earns words?** Proposed rule: a label earns a word, a number earns no
  explanation, and a paragraph must be doing work no chart or chip can do. Most
  current subtitles fail that test.
- **What is the density?** This is an instrument for people who want to see their
  machine — not a marketing page. Density is correct; *undifferentiated* density
  is the defect. The fix is hierarchy, not emptiness.
- **What does premium mean here?** Name it concretely: typography carrying the
  hierarchy so the structure survives with colour removed; restraint with the
  accent; one idea per region; generous space between groups rather than inside
  them; motion only where it reports state.
- **What does the orb teach?** It is the one element that shows instead of tells.
  Say what makes it work and generalise it.

**1.C — rebuild the Overview** to that direction. It is the screen the owner
judges the product on and it has been through three passes already — the previous
one, P51, brought it from 4,110 px to 1,715 px and got the composition right. This
pass is about **what it says**, not where things sit.

Ruthless on copy: cut every sentence that describes the UI, names the obvious, or
restates a number. Keep every sentence that carries a fact the reader cannot get
elsewhere. Report the word count before and after.

**1.D — stop and present it.** Screenshots at 1280, both languages, both themes,
populated and empty. **Do not roll out to other screens until the owner approves
the direction.** Three previous design passes were rejected; one screen is the
cheapest way to find out whether this one lands.

---

## PART 2 — the local AI surface

**Measured, so nobody re-derives it:** there is **no chat**. The only AI-facing
wire verbs are `InsightsList`, `InsightsExplain` and `InsightsDismiss`, and
`grep -c chat services/maintenance-service/src/*.rs` returns nothing. Commit
`2be8396` is titled "finish local AI chat" and did not deliver one — record that
as a debt id so the next session does not trust the message.

### The constraint that decides the whole feature

This product drops any insight that cannot cite its evidence. A free-form chat
with an embedded 1.5B model is precisely what that rule exists to prevent: the
user asks "why is my PC slow?" and the model answers fluently, from training, with
no measurement behind a single clause.

**So the chat must be grounded.** It answers from the evidence the product has
collected, and every claim carries the chip that proves it. When the question
cannot be answered from collected evidence, the correct answer is to say so and
name the scan that would produce it — not to speculate.

That constraint is not a limitation to work around. It is the feature: an
assistant that can only tell you things about your machine that it can prove is
something no general chatbot is.

**2.A — design the interaction before the wire.** Where does it live: a panel, a
dedicated screen, a command-palette mode? What does the empty state say — what
can it answer today, given what has been scanned? What happens to a question it
cannot ground? Write it in `DIRECTION.md` and argue for it.

**2.B — the contract.** Add the verb to the proto. It is a wire change, so it is a
published-contract change: state the shape, the streaming behaviour, and the
cancellation path before implementing. Nothing has shipped to a user, so breaking
the wire is free today and expensive after the first release — do it properly now.

**2.C — the service side.** Route to the embedded model. Bound it: a deadline, a
token ceiling, cancellation, and a refusal path when the model is unavailable that
is a **declared fault with a reason**, never an empty answer. `engineLabel` must
keep reporting `localModel` and never silently fall back.

**2.D — the UI**, built in Part 1's language. Streaming, cancellable, keyboard-first.
Every response carries its evidence chips. A response with no citable evidence
renders as the honest refusal, not as prose.

**2.E — the tests that would catch the failure that matters.** Commit them failing
first, as this project does. At minimum: a question with no supporting evidence
must NOT produce a confident answer; a model failure must surface a fault rather
than an empty string; and cancellation must actually stop generation.

---

## PART 3 — roll out. Only after the owner approves Part 1.

Apply the direction to the remaining screens, two at a time, committing each pair.
For each: word count before and after, height before and after, and a screenshot.

If a screen's content cannot survive the copy cut without losing a fact, **keep
the fact and say so**. Do not delete information to hit a style.

---

## VERIFY — measure, do not assume

    node tools/verify-numbers.mjs   EXPECTED: zero untraceable numbers
    node tools/verify-arabic.mjs    EXPECTED: 7/7, zero system-font fallback
    node tools/verify-tokens.mjs    EXPECTED: every var() resolves
    tools/layout-sweep.mjs          EXPECTED: clean at 1280/1024/960, both
                                    languages, BOTH themes, populated AND empty
    the contrast instrument         EXPECTED: nothing below 4.5:1, either theme

Build passes; `svelte-check` stays at its baseline.

**Exercise every screen with real data and with none.** The light theme was broken
for months because the sweep ran dark-only, and four responsive screenshots once
passed only because there was no data to overflow.

Verify Arabic from the rendered DOM, never from a screenshot: a previous session
judged Arabic from an image and reported a cause the HTML disproved, while 1,445
of 1,795 characters were being painted by Tahoma.

---

## RULES

- Every check has an **EXPECTED** value. Observed differs → stop that item, record
  the raw observation verbatim, move to the next independent item.
- Explicit paths when staging. **Never `git add -A`.**
- Commit and push after every item; move the LEDGER row in the same commit.
- End every commit with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- No new colour literals. Everything derives from the token layer.
- The icon is settled — the Æ mark. Do not revisit it.
- `DBT-P55-005` says the mark's source SVG is missing from the tree while the
  generated PNGs match their hashes. If you can restore the source from the design
  worktree, do it; if not, leave it and say so.

---

## REPORT

- the before/after measurement table: words of prose, type sizes, elevations, and
  the text-to-data ratio, per screen
- `DIRECTION.md`, and the one screen that proves it
- the chat's interaction design, and what it does with a question it cannot ground
- the raw gate output, counts not adjectives
- screenshots at 1280, both languages, both themes, populated and empty
- one paragraph: what the interface now shows that it used to explain
