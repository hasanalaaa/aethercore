# P56 — the direction

The stance has not changed since `apps/ui/DESIGN_LANGUAGE.md` was written:
**evidence-first technical calm**. What changes here is that the stance now has
numbers attached, because three design passes were rejected while every rule in
that file was, on paper, being followed.

Everything below is measurable by `apps/ui/tools/measure-density.mjs` or by the
gates already in the tree. A principle you cannot fail is a preference.

---

## The five principles

### 1. One hero per screen, and a scale you can count on one hand

A screen has exactly **one** figure at hero scale — the number the screen is
about. Everything else steps down through the token scale, and **nothing on a
screen may use a size that is not in `design-tokens.css`**.

> Target: **≤ 6 distinct type sizes** per screen, every one of them a token.
> Overview today: 15, of which 8 are literals.

Why this and not "less text": with fifteen sizes, five of them within 1.5px of
each other, the reader gets no signal about what to read first, so they read
everything, and *everything* feels like too much. The words were never the
problem on this screen — 63 of them, populated. The absence of rank was.

### 2. A word must be a fact

- A **label** earns one word, two if the noun genuinely needs it.
- A **number** earns no explanation. If the reading needs a sentence, the reading
  is wrong or its unit is missing.
- A **paragraph** must carry a fact that no chip, meter, chart or empty state can
  carry. Everything that describes the interface, names the obvious, or restates
  a number next to it is deleted.

The test is mechanical: read the sentence, then ask *what does the reader now know
that the screen did not already show them?* If the answer is "how this screen
works", it goes.

> Target on the Overview: **≤ 30 prose words populated** (from 63) and **≤ 45
> empty** (from 167). Words per reading, empty: **≤ 1.5** (from 5.22).

The empty number matters more than the populated one. Empty is where this product
explains itself hardest — the whole app more than doubles its prose when it has
nothing to show — and empty is the first thing the owner sees.

### 3. Density is right; undifferentiated density is the defect

This is an instrument for someone who wants to see their machine. Nothing here is
removed to make the screen emptier. What is removed is *sameness*: surfaces that
all weigh the same, sizes that all sit within two points of each other, groups
separated by the same gap that separates their contents.

**Surfaces carry structure, not decoration.** Three neutral levels, and no more:

| level | what it is | where |
|---|---|---|
| **ground** | the page | `--ac-canvas` |
| **card** | one region of the instrument | `--ac-material-base` |
| **well** | something recessed *inside* a card — a log, a readout | `--ac-sunken` |

A card never sits inside a card. Role-tinted fills — a critical chip, a healthy
tag, the violet of a policy refusal — are **semantics, not elevation**: they are
allowed to be plural, and they never nest.

> Target: **3 neutral surface levels** on the Overview (from 6). Role-tinted
> fills are not capped; they mean something.

**Space groups, it does not pad.** The gap between two sections is larger than
any gap inside one. Concretely, on a card: `--ac-space-5` between a card's own
parts, `--ac-space-7` between cards. Today the Overview uses `--ac-space-4`
between cards and `--ac-space-5` inside them — the grouping is inverted, which is
why nine regions read as one field of boxes.

### 4. Premium is typography carrying the hierarchy

Named concretely, so it can be checked:

- **The structure survives with colour removed.** Screenshot in greyscale; if you
  can no longer tell the primary reading from its caption, the hierarchy was
  being carried by hue and it fails. Colour then goes back to doing only what it
  is for here: the six roles, and the violet that means *refused on purpose*.
- **Restraint with the accent.** One solid interactive colour, never a gradient,
  and it appears only on things you can act on. A meter fill is interactive-hued
  because it reports a live reading; a decorative wash on a card is not.
- **One idea per region.** A section answers one question. If it answers two, it
  is two sections or it is a table.
- **Generous space between groups, tight inside them.** Principle 3's rule, said
  as an aesthetic.
- **Motion only where it reports state.** The log cursor blinks because the log is
  live. The orb's ring reads differently when it has a sample. Nothing else moves,
  and everything that does is off under `prefers-reduced-motion`.

Explicitly **not** premium here: more whitespace, thinner type, bigger radii,
glass. This product is air-gapped and reads counters; a marketing page's idea of
luxury would be a lie about what it is.

### 5. What the orb teaches

The orb is the one element on the screen that **shows instead of tells**, and the
owner is right to like it. What actually makes it work, generalised into rules
the rest of the product must follow:

1. **It states a reading, not a conclusion.** It shows headroom, which is
   measured, not "health", which would be a judgement the service never made. Its
   own source comment says so: there is no state hue, because "a health threshold
   is not a measurement".
2. **It has an honest zero.** With no sample it renders `—` and its ring stays
   neutral. It never invents a number to look alive.
3. **It needs no sentence.** Value, one-word caption, one state. Cover the caption
   and you still know what you are looking at.
4. **Its scale is its rank.** It is the biggest thing on the screen because it is
   the most important thing on the screen. That is the whole hierarchy, done once,
   with type.

The generalisation: **every region gets one element that shows, and the words
around it only name it.** Where a region has no such element, that region is
prose pretending to be a feature, and it should be a table, a meter, or gone.

---

## What must not change, restated as build constraints

These come from the brief and are guarantees, not taste. They are listed here so
the roll-out in Part 3 cannot quietly trade one away for a cleaner screen.

- **DENIED BY POLICY** keeps the violet identity, the dashed 1.5px border, the 8px
  radius and its own icon, and never shares error styling.
- Every insight carries its evidence chip, expandable to the raw observation.
- Honest empty states: `pk("Not collected yet", "لم تُجمع بعد")`. A meter at rest
  shows `—`. No fake zero, no invented score, no scare copy.
- The persistent policy band stays.
- Arabic is authored at source, `pk("EN","AR")` at every string, font bundled
  locally. A CDN would break the air-gap invariant, not merely slow the app.
- Six colour roles keep their meanings. Interactive is solid, never a gradient.
- No number that cannot be traced to a measurement.

One direct consequence for Part 3: if a screen's copy cannot survive the cut
without losing a fact, **the fact stays and the row says so.** Principle 2 deletes
descriptions of the interface, never observations about the machine.

---

## Part 2.A — the local AI surface: where it lives and what it refuses

### The measured starting point

There is **no chat**. The AI-facing wire verbs are `ListInsights`,
`RequestInsight` and `DismissInsight`; `grep -c chat services/maintenance-service/src/*.rs`
returns zero on all fourteen files. Commit `2be8396` is titled "finish local AI
chat" and did not deliver one — recorded as `DBT-P56-001` so the next session does
not trust the message.

A second thing was measured while reading the path, and it is worse:
`LlamaCppReasoner::infer_embedded` (`crates/intelligence-core/src/llama.rs`)
**returns `Err` unconditionally** — "token-level generation requires the context
pool wired in intelligence.rs". The embedded model verifies its hash, loads, and
creates a context at startup, and then every inference request falls through to
`DeterministicFallbackReasoner`. The product has been reporting a local model that
has never generated a token. Recorded as `DBT-P56-002`.

### The constraint is the feature

This product drops any insight that cannot cite its evidence. A free-form chat
with an embedded 1.5B model is exactly what that rule exists to prevent: ask "why
is my PC slow?" and a small instruct model will answer fluently, from training,
with no measurement behind a single clause.

So the assistant answers **only** from the evidence the product has collected,
and every claim carries the chip that proves it. That is not a limitation worked
around — an assistant that can only tell you things about *your* machine that it
can *prove* is something no general chatbot is, and it is the only kind of
assistant this product is allowed to ship.

### Where it lives: a drawer, on every screen

Three options were considered.

| option | why not |
|---|---|
| **a dedicated screen** | the questions people ask are about the screen they are looking at — "why is this driver flagged?" — and a screen forces them to leave the evidence behind to ask about it |
| **a command-palette mode** | the palette is a one-shot navigator: type, choose, it closes. A conversation needs a transcript that persists while you look at something else, and the palette has nowhere to put one |
| **a docked column** | at 1280 the content area is already at its comfortable floor; a permanent 26rem column would push the instrument below the breakpoint where the Overview collapses to one column |

**Decided: an overlay drawer on the inline-end edge**, 26rem wide, opened with
`Ctrl+/` or from the rail, dismissed with `Escape`. It overlays rather than
reflows, so no screen changes layout because the assistant is open, and below
the `58rem` container breakpoint it becomes a full-width sheet like every other
overlay in the shell. It is keyboard-first: `Ctrl+/` opens and focuses the input,
`Enter` sends, `Escape` cancels a streaming turn and then closes, `Tab` reaches
every citation chip.

### The empty state says what it can answer *today*

Not a list of capabilities — a count of evidence. The drawer opens showing the
surfaces that currently have rows and how many:

> `maintenanceHistory 8` · `timelinePattern 12`

and nothing else. When there are none, it is the ordinary honest empty state —
`pk("Not collected yet","لم تُجمع بعد")` — naming the scans that would produce
evidence, with the routes to them. No capability tour, no example questions the
product cannot actually answer.

### A question it cannot ground

**It says so, and names the scan.** Concretely, three distinct outcomes, and the
distinction between them is a product guarantee, not a nicety:

| outcome | what it means | how it renders |
|---|---|---|
| **answered** | the model produced text, every claim resolved to a citation in the pack | the answer, with its evidence chips |
| **refused** | nothing in the collected evidence bears on the question | the honest empty state, inside the transcript, naming the scan that would produce the evidence. **Not** error styling |
| **faulted** | the model was unavailable, exceeded its deadline, or emitted something unparseable | a declared fault with its reason key. **Never** an empty answer, never a silent degrade to the rule engine |

And separately: **DENIED BY POLICY keeps its own identity here too.** An
ungroundable question is *not* a policy refusal — the product is not protecting
you from it, it simply has not measured the thing. Rendering it in violet would
spend the one visual signal that means "we refused on purpose" on a case where
nothing was refused. Refusal-for-lack-of-evidence gets the empty-state treatment;
violet stays reserved.

The ungrounded prose itself is **never shown**. If the model writes a paragraph
and none of its citations resolve against the pack, the paragraph is discarded
and the turn becomes a refusal. Showing it greyed, or with a "low confidence"
badge, would put an uncited claim about the user's machine on the screen, which
is the exact thing this product exists not to do.

### The wire, stated before it is implemented

A wire change is a published-contract change (§4 of the operating contract).
Nothing has shipped to a user, so breaking it is free today and expensive after
the first release. The shape:

```proto
// assistant.proto
message AskAssistantRequest {
  string turn_id  = 1;   // client-assigned; echoed on every delta and the terminal turn
  string question = 2;   // free-form, bounded by MAX_QUESTION_CHARS
}
message CancelAssistantTurnRequest { string turn_id = 1; }

enum AssistantTurnState {
  ASSISTANT_TURN_STATE_UNSPECIFIED = 0;
  ASSISTANT_TURN_STATE_STREAMING   = 1;
  ASSISTANT_TURN_STATE_ANSWERED    = 2;
  ASSISTANT_TURN_STATE_REFUSED     = 3;
  ASSISTANT_TURN_STATE_FAULTED     = 4;
  ASSISTANT_TURN_STATE_CANCELLED   = 5;
}
enum AssistantRefusalReason {
  ASSISTANT_REFUSAL_REASON_UNSPECIFIED   = 0;
  ASSISTANT_REFUSAL_REASON_NO_EVIDENCE   = 1;  // nothing has been collected at all
  ASSISTANT_REFUSAL_REASON_NOT_COVERED   = 2;  // evidence exists; none of it bears on the question
  ASSISTANT_REFUSAL_REASON_MUTATION_ACTIVE = 3;// observer-effect guard (I4)
  ASSISTANT_REFUSAL_REASON_BUSY          = 4;  // single-flight lane held
}

message AssistantEvidenceRef {
  string evidence_id = 1;
  string surface     = 2;  // the four EvidenceSurface names, unchanged
  string detail      = 3;  // the raw observation the chip expands to
}
message AssistantTurn {
  string turn_id = 1;
  uint32 schema_version = 2;
  AssistantTurnState state = 3;
  string answer = 4;                            // accumulated text so far, never a fragment
  repeated AssistantEvidenceRef citations = 5;
  string engine_label = 6;                      // localModel | ruleFallback | disabled
  AssistantRefusalReason refusal = 7;
  string fault_key = 8;                         // assistant.fault.* when state = FAULTED
  uint32 tokens_emitted = 9;
  repeated AssistantEvidenceRef pack = 10;      // what the turn was allowed to draw on
}
```

**Streaming.** `AskAssistant` returns immediately with an `AssistantTurn` in
`STREAMING`. Generation then pushes `EVENT_KIND_ASSISTANT_TURN` envelopes
carrying the **accumulated** answer — not deltas — so a client that missed an
event is never left with a torn sentence, and the stream's existing replay and
sequence machinery applies unchanged. Exactly one terminal event follows, in
`ANSWERED`, `REFUSED`, `FAULTED` or `CANCELLED`.

**Grounding is enforced at the terminal edge, not per token.** The citation gate
can only run on a complete answer, so streamed text is rendered as provisional
and the terminal turn either confirms it with citations or replaces the whole
turn with a refusal. The UI must therefore not treat streamed text as an answer
until the terminal state arrives — stated here because it is the one place a
careless client could put an uncited claim on screen.

**Cancellation** is its own verb, `CancelAssistantTurn`, not the session's
`CancelRequest`: the request has already returned by the time the user presses
Escape, so cancelling the *request* would cancel nothing. The verb raises a flag
the generation loop checks between tokens; the turn terminates `CANCELLED` with
whatever it had, and the lane is released. "It stops generating" is asserted by a
test, not by the flag existing.

**Bounds** (all named constants, all asserted): question ≤ 2,000 chars; prompt ≤
the existing 8,192-char context contract; ≤ 512 generated tokens; a 20-second
wall-clock deadline; one turn in flight per principal; no persistence — the
transcript is session state in the renderer and dies with the window, exactly as
insights do.

**`engineLabel` never lies.** It reports `localModel` when the embedded reasoner
is loaded and `disabled` when it is not. It does **not** silently become
`ruleFallback` on a generation failure: a failure is a `FAULTED` turn with a
reason. The rule engine summarises evidence; it cannot answer a question, and
letting it answer one under the model's label is the silent fallback the brief
forbids.

---

## Part 3 — the numeral convention (P57 ITEM 2)

**Decided: Latin digits for every reading this product renders, in both
languages, including the service log.** One convention, product-wide.

The product was rendering two. In Arabic, `formatNumber` and `formatDateTime`
resolved to `ar-IQ`, so a reading printed `٥١٪` and `١٢٬٤٨٠`, while the service
log printed `seq 1` and `12:30:00` and the Timeline screen had already
hard-coded `numberingSystem: 'latn'` in a formatter of its own. Three
behaviours, one screen apart.

### Why Latin, measured rather than preferred

This is an instrument. Its numbers are scanned, not read, and most of them sit
in monospace beside Latin identifiers, paths, digests and versions that cannot
change: `NET-NO-EGRESS`, `oem214.inf`, `plan-0001`, `0.1.0`. So the test is
whether a number and the identifier beside it can share a line.

Measured on this machine, at 20px, through the app's own `--ac-font-mono` stack
in the `ar` locale (`CSS.getPlatformFontsForNode`, which reports what the
compositor actually drew — not what the stack asked for):

| run | width | drawn by |
|---|---:|---|
| `12,480` | 72.00px | JetBrains Mono — 6 glyphs |
| `١٢٬٤٨٠` | 46.59px | IBM Plex Sans Arabic — 6 glyphs |
| `seq 1 12:30:00` | 168.33px | JetBrains Mono — 14 glyphs |
| `seq ١ ١٢:٣٠:٠٠` | 134.09px | JetBrains Mono **7** + IBM Plex Sans Arabic **7** |

The last row is the finding. **JetBrains Mono has no Arabic-Indic digits**, so
inside a single technical token the identifier draws monospaced and the number
beside it draws from a proportional face. The monospace grid — the entire reason
a reading is set in mono — breaks mid-token. The same six characters are 35%
narrower in one numbering system than the other, so columns of readings cannot
align either.

With a bare `"JetBrains Mono", monospace` stack the same string falls all the
way through to **Courier New, a system font** — the exact class of defect
`verify-arabic` exists to catch.

### What this does NOT change

`-u-nu-latn` changes the digits and nothing else. The locale keeps its own
conventions:

* unit symbols stay Arabic — `84°م`
* date order and the AM/PM marker stay Arabic — `14/04/2026، 12:30 م`
* plural agreement is unchanged; `Intl.PluralRules` selects from the value, not
  from its spelling

This is one numbering system for an instrument, not Arabic rendered as English.

### One gate widened as a consequence

CLDR wraps a percent in LRM under an RTL locale — `51‎%‎`. `verify-numbers`
matched `\d+\s*%`, so every Arabic percentage would have become invisible to the
gate the moment the digits turned Latin: a number nobody could trace, hidden
from the instrument by a zero-width character. The matcher now treats bidi
control marks as separators.
