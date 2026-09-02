# AetherCore — design brief v3

Paste into Claude Design (or Claude Code with the file attached):
`Read and follow DESIGN-BRIEF-V3.md. The file to improve is AetherCore.html.`

---

## WHAT YOU ARE WORKING ON

`AetherCore.html` in this directory — a single bundled interactive page, ~5.9 MB.
It already works and much of it is right. **This is a revision, not a restart.**

What already works and must survive:

- **Bilingual at the string level.** Every label is `pk("English", "العربية")`.
  Arabic is not a translation layer bolted on — it is authored at source. Keep
  this, and add Arabic for every new string you write. No exceptions.
- Honest empty states — `pk("Not collected yet", "لم تُجمع بعد")`.
- Consent copy that names reversibility explicitly.
- `JetBrains Mono` reserved for machine facts.
- Real depth: `--glass`, `--shadow: 0 24px 60px -22px rgba(0,0,0,0.75)`.

---

## THE PRODUCT — read this before you touch a colour

AetherCore is a local-first system maintenance **instrument** for Windows.

- Air-gapped by construction. No account, no login, no subscription, no telemetry,
  no server, no network at rest.
- An AI model runs **on the machine**, embedded, pinned by hash.
- It diagnoses **read-only** before it ever mutates anything.
- **Every insight cites the evidence it came from. An insight that cannot cite is
  dropped before it is displayed.**
- **It refuses certain operations deliberately — as a guarantee to the user.**
- Every mutation is governed, consented, and reversible.

That last pair is the whole product. Nothing else on the market says *"I will not
do that, on purpose, and here is the rule that stopped me."*

---

## WHAT IS MISSING — measured in the current file, not guessed

    "denied" / "DENIED"       0 occurrences
    violet / purple / #7C6BC4 0
    "evidence"                0
    "seal" / gold #C9A227     0

Two of the four signature elements are absent, and the most important colour rule
in the system was dropped. The result is a **beautiful, generic system dashboard**.
It could belong to any product. It does not look like this one.

### 1. DENIED BY POLICY — its own visual family

A refusal is not an error. An error means something broke. A refusal means the
product **worked exactly as promised**. They must never share styling.

Today there is no state for it at all. Add one:

- its own hue, distinct from the success / warning / critical family and distinct
  from the interactive accent
- a treatment that reads as *deliberate and settled*, not alarming — a refusal is
  calm, it is the system holding a line
- it always shows **which rule** refused, in mono, next to the refusal
- it appears in the persistent policy band, so the user always knows what the app
  is currently permitted and not permitted to do

Design the state that a user should feel reassured to see.

### 2. THE EVIDENCE CHIP

Every insight carries a compact affordance that attaches a claim to its source —
a counter name, a file path, an event id, a timestamp. Small, quiet, always
present, expandable to the raw observation.

This is the visual proof of the product's core promise. It should be one of the
first things a designer would point to in a portfolio shot.

### 3. THE VERIFIED SEAL

A distinct treatment, used **only** beside something cryptographically verified —
a signed manifest, a hash-pinned model, a signed licence. Rare by design. Its
scarcity is what gives it meaning. Never decorative.

### 4. RESTORE THE MEANINGS

Six roles, each visually distinguishable from the other five at a glance and to a
colour-blind user:

    interactive · verified · healthy · attention · critical · denied-by-policy

**You may change the palette.** The current hexes are not sacred — the *meanings*
are. If you can build a more sophisticated palette that keeps six distinguishable
roles, do it, and write down what each role is and why you chose it. What you may
not do is let denied-by-policy fall into the error family, or let interactive
double as a state.

---

## THE BAR — this is the part that matters

The design must read as the work of a team with a point of view, not a model
producing a competent dashboard. Concretely:

### Have an actual thesis

This is an **instrument**, not a SaaS dashboard. Instruments have a specific
visual culture: precision, calibration, restraint, legibility under pressure,
labels that say exactly what a channel measures. Look at oscilloscopes, aviation
panels, audio metering, scientific software — not at admin templates.

Write your thesis in one sentence before you design, and make every choice
traceable to it.

### The tells that give away generic work — avoid every one

- gradients used as decoration rather than to encode something
- glass on glass on glass until nothing has a ground
- a centred hero with one enormous number and nothing supporting it
- emoji, or icon sets that don't share a drawing logic
- every card the same size, the same radius, the same elevation
- effects doing the work typography should be doing
- an accent colour sprayed across the interface until it stops meaning anything
- copy that describes the UI ("Dashboard Overview") instead of saying something
- symmetry everywhere, hierarchy nowhere

### What separates real design work

- **Typography carries the hierarchy.** Weight, size, spacing and case do the
  work. If you removed all colour, the structure should still be legible.
- **Restraint with the accent.** One accent, used where a decision is made.
- **Density where density is honest.** This user wants to see their machine. Do
  not pad an information-dense product into a marketing page.
- **The empty state is the hero.** It is the first thing every new user sees, and
  it is what the owner has rejected twice. A screen with no data must still feel
  like a precision instrument at rest — labelled channels awaiting their first
  reading — never a shrug, never a dashed rectangle with one icon in it.
- **Arabic is a first-class layout**, not a mirrored afterthought. Check that the
  RTL composition is genuinely designed, and that letters join correctly with a
  real Arabic-capable font loaded locally.

---

## HONESTY — a product invariant, not a style preference

- No invented number. No fabricated precision. No score with no measurement behind
  it. A previous pass showed "ENGINE INTEGRITY — 94.2%" with a progress bar. That
  is the single worst thing this design could do.
- No scare copy. No "imminent risk detected". The product does not frighten people
  into clicking.
- Empty means empty. `pk("Not collected yet", "لم تُجمع بعد")` — never a zero
  standing in for an unknown.

If a number appears on screen, be able to say which measurement produced it.

---

## SCOPE AND VERIFICATION

Work in `AetherCore.html`. Keep it a single self-contained file.

Priority order — do not start the next until the previous is genuinely done:

1. denied-by-policy as a real state, everywhere it belongs
2. the evidence chip, on every insight
3. the palette with its six meanings written down
4. the verified seal
5. the empty state, refined until it is the best screen in the set

Verify before you report — measure, do not assume:

- grep your own output: `denied`, `evidence`, and the new palette roles must
  appear. Report the counts.
- render at **1280, 1024 and 960 px**. A previous pass had a button physically
  covering body text at 960. EXPECTED: no overlap, no horizontal scroll, no
  clipped text at any width.
- render the Arabic view and confirm from the rendered DOM — not from a
  screenshot — that the layout is RTL and the letters join.
- grep for any percentage or score not traceable to a measurement. EXPECTED: zero.

Deliver the file, screenshots at all three widths in both languages, and one short
paragraph: your thesis, what you changed in the palette and why, and what you
deliberately left alone.
