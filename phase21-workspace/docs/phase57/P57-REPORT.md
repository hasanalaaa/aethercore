# P57 — the drawer, the numerals, and the whole product

Everything below was measured, not judged. The instruments are
`apps/ui/tools/{measure-density,layout-sweep,verify-numbers,verify-arabic,verify-tokens,contrast-sweep}.mjs`,
and every one of them reads the rendered DOM. Raw measurements:
`docs/phase56/measure/p57-{before,after}-{populated,empty}.json`.
Screens: `docs/phase57/screens/`. The drawer: `docs/phase56/screens/assistant/`.

The baseline labelled "before" is the **P56 tree**, taken at the start of this
phase — not the P55 tree `DENSITY.md` measured. That keeps the Overview's
rebuild out of P57's numbers.

---

## ITEM 1 — the assistant drawer

An overlay on the inline-end edge, 26rem, fixed to the viewport so **no screen
changes layout because the assistant is open**. Full-width sheet below 58rem.
`Ctrl+/` opens and focuses the input, `Enter` sends, `Escape` cancels a streaming
turn and then closes, `Tab` reaches every citation chip, and the rail carries a
trigger beside the command palette. Available on every screen; it survives
navigation, because the question is about the screen you went to look at.

### The invariant, and where it is enforced

| outcome | how it renders | what stops it being prose |
|---|---|---|
| **streaming** | muted ink, a live cursor, no answer label, **no citation row** | the renderer keys off `state`, never off whether text is present. The citation gate only runs on a complete answer |
| **answered** | the answer, its `[En]` markers as inline references, and an `EvidenceChip` per citation that expands to the raw observation | `citations` is non-empty exactly when the state is ANSWERED |
| **refused** | the honest empty state, inside the transcript, naming the scan that would produce the evidence | **not** error styling, and **not** violet. Nothing was refused on purpose, so DENIED BY POLICY keeps its one signal. **0 denied chips in all 56 measured drawer states** |
| **faulted** | a declared fault with its reason key, in the critical role | never an empty answer; an unknown key falls back to the generic failure rather than to `''` |
| **cancelled** | the tokens it reached, as a reading | cancellation raises the flag the generation loop reads between tokens; the terminal envelope settles the turn |

### The empty state counts evidence, not capabilities

    maintenanceHistory 8 · timelinePattern 12

in the order `compose_evidence_pack` composes them. Reading the pack before a
turn exists needed one additive, read-only wire verb — `GetAssistantPack`,
request tag 94, response tag 93 — answered from the **same**
`compose_evidence_pack` the insight and turn paths use. Two compositions would
mean two answers to "what has this product measured?".

### Measured, from the rendered DOM

    drawer states          56/56 clean
                           2 widths x 2 languages x 2 themes x 7 states
    overflowX              0 in every state
    clipped boxes          0 in every state
    geometry               416px at x=864 of 1280 (LTR) · x=0 (RTL)
                           880px full-width sheet at 880
    type sizes             3-4 per state, every one a token
    neutral surfaces       3 (focused ground · sunken well · glass control)
    denied chips           0
    evidence chips         2 on an answered turn, expanding to the raw row
    Escape cancels, Escape closes    8/8
    verify-arabic --page assistant   7/7, 0 glyphs from a system fallback
    contrast-sweep --drawer 1        756 nodes, 0 below 4.5:1

Two instruments were widened, because both were blind to a shell overlay the
same way `layout-sweep` had been blind to eleven screens: `verify-arabic` gained
`--page assistant`, `contrast-sweep` gained `--drawer 1`.

### One defect found while measuring

Assigning to a **member** of a reactive `let` invalidates that variable in
Svelte, so `transcriptEl.scrollTop = transcriptEl.scrollHeight` re-triggered the
reactive statement that called it. The renderer stopped answering CDP entirely
on the first question. Fixed by reading the element into a local first.

---

## ITEM 2 — the numeral convention

**Decided: Latin digits for every reading, in both languages, including the
service log.** The full argument is in `docs/phase56/DIRECTION.md` Part 3.

The product was rendering three conventions: `ar-IQ` gave `٥١٪` and `١٢٬٤٨٠`,
the service log gave `seq 1` and `12:30:00`, and the Timeline screen had already
hard-coded `numberingSystem: 'latn'` in a formatter of its own.

Measured through the app's own `--ac-font-mono` stack in the `ar` locale, at
20px, asking the compositor what it actually drew:

| run | width | drawn by |
|---|---:|---|
| `12,480` | 72.00px | JetBrains Mono — 6 glyphs |
| `١٢٬٤٨٠` | 46.59px | IBM Plex Sans Arabic — 6 glyphs |
| `seq 1 12:30:00` | 168.33px | JetBrains Mono — 14 glyphs |
| `seq ١ ١٢:٣٠:٠٠` | 134.09px | JetBrains Mono **7** + IBM Plex Sans Arabic **7** |

JetBrains Mono has no Arabic-Indic digits, so inside **one** technical token the
identifier draws monospaced and the number beside it draws from a proportional
face. The monospace grid breaks mid-token, and the same six characters are 35%
narrower in one system than the other, so columns of readings cannot align.

`-u-nu-latn` changes the digits and nothing else: units stay Arabic (`84°م`), and
so do date order and the AM/PM marker (`14/04/2026، 12:30 م`).

---

## ITEM 3 — the twelve screens

| screen | type sizes | neutral surfaces | prose populated en/ar | prose empty en/ar | words per reading (pop en) | height 1280 en |
|---|---|---|---|---|---|---|
| overview | 6 → **6** | 3 → **3** | 0→**0** / 0→**0** | 24→**5** / 16→**0** | 0 → **0** | 1578 → **1578** |
| deepScan | 13 → **6** | 2 → **2** | 230→**166** / 221→**157** | 55→**5** / 43→**0** | 16.43 → **11.07** | 1428 → **1411** |
| drivers | 10 → **5** | 6 → **3** | 103→**65** / 127→**90** | 91→**23** / 83→**19** | 1.66 → **1.02** | 2096 → **1933** |
| repair | 10 → **6** | 7 → **3** | 166→**55** / 137→**21** | 110→**5** / 106→**0** | 18.44 → **5** | 1535 → **1247** |
| cleanup | 8 → **5** | 3 → **2** | 199→**125** / 117→**32** | 104→**10** / 111→**11** | 9.95 → **5.95** | 1400 → **1144** |
| startup | 8 → **5** | 3 → **2** | 145→**73** / 97→**16** | 107→**5** / 111→**5** | 6.59 → **3.32** | 1841 → **1636** |
| performance | 8 → **5** | 2 → **3** | 89→**12** / 76→**17** | 75→**5** / 60→**5** | 6.85 → **0.8** | 1226 → **980** |
| hardware | 10 → **5** | 3 → **3** | 219→**87** / 160→**38** | 100→**5** / 92→**5** | 2.92 → **1.16** | 2215 → **1949** |
| crash | 9 → **5** | 2 → **2** | 169→**85** / 123→**34** | 143→**28** / 126→**17** | 8.89 → **4.47** | 1679 → **1456** |
| activity | 10 → **6** | 3 → **3** | 142→**69** / 121→**52** | 184→**36** / 156→**18** | 8.88 → **4.06** | 1971 → **1865** |
| fleet | 3 → **3** | 1 → **1** | 6→**0** / 8→**0** | 36→**11** / 24→**0** | 1.5 → **0** | 900 → **900** |
| settings | 10 → **6** | 4 → **2** | 65→**19** / 70→**18** | 107→**24** / 99→**18** | 3.1 → **0.9** | 1388 → **1273** |
| **TOTAL** | | | 1533→**756** / 1257→**475** | 1136→**162** / 1027→**98** | | |

Words per reading, whole product: **4.20 → 2.02** populated (en),
**3.13 → 1.16** (ar); **9.09 → 1.13** empty (en), **8.28 → 0.69** (ar).

The sidebar, constant on every screen: prose **47 → 10**, type sizes **9 → 3**.

### What was kept, and why

The brief's rule — *delete explanation, never information* — kept five things a
cut would have removed:

| kept | words | the fact it carries |
|---|---:|---|
| Hardware's memory truth note | 7 | "No logged memory error ≠ RAM proven healthy", under a counter reading 0. Without it that 0 reads as proof |
| Crash History's "why no culprit is named" | 10 | the epistemic limit of the evidence on that screen, and the reason it never names one |
| the timeline's correlation note | 4 | "Correlation is not causation", over every pattern on the screen |
| the support bundle's copy | 9 | what is **not** in the bundle, decided before a preview exists |
| System Repair's escalation card | 11 | a recommendation exists that no in-product action can execute |

Two more were kept whole because the service, not the UI, owns their wording:
Deep Scan's finding summaries (**166 of the 756 remaining words**), and Startup's
per-item `evidenceDetail` — "Observed adding 1.9 s to the last five sign-in
traces" — which is a measurement this product took.

### What was deleted, by category

| category | screens | words |
|---|---|---:|
| eyebrow + subtitle above a title | all 11 | ~180 |
| a safety/policy paragraph restating the persistent policy band | performance, repair, cleanup, startup, hardware | ~150 |
| an empty-state body narrating how the screen works | all 11 | ~120 |
| the rail's twelve descriptions | every screen, permanently | 47 x 12 renders |
| the disconnected card's body, which restated its own heading | every screen, empty | 19 x 12 renders |
| a heading that was a sentence | activity, drivers, crash, hardware, repair, cleanup, startup | — |

**71 catalog keys were removed in both languages** rather than left orphaned; 23 were added, 21 of them the drawer's. Net: 1,625 → 1,577.

---

## The gates, observed

```
node tools/verify-numbers.mjs    34 distinct score-shaped numbers, each traced
                                 0 untraceable
node tools/verify-arabic.mjs     7/7 pass; 0 glyphs from a system fallback font
  ... --page assistant           7/7 pass; 0 glyphs from a system fallback font
node tools/verify-tokens.mjs     PASS — 741 var() declarations, 0 unresolved
node tools/layout-sweep.mjs      144/144 populated · 144/144 empty
                                 12 screens x 2 languages x 2 themes x 3 widths
  the drawer                      56/56 states clean
node tools/contrast-sweep.mjs    5,682 text nodes, 0 below 4.5:1, both themes
  ... --drawer 1                   756 text nodes, 0 below 4.5:1
npx svelte-check                 0 errors, 0 warnings (229 files)
pnpm build                       built in 659ms
cargo check                      exit 0
```

---

## Pre-existing, with the evidence that it predates this phase

Every count below is taken from commit `a540d75`, the P56 tip, before the first
P57 commit.

| finding | evidence at `a540d75` | fixed here? |
|---|---|---|
| `verify-numbers` matched `\d`, which never matches `٥١` — **every Arabic percentage was invisible to the numbers gate** | `tools/verify-numbers.mjs:76`, `/(\d+(?:[.,]\d+)?)\s*%/g` | **yes**, as a consequence of ITEM 2. The gate went from 27 traced numbers to 34; the 7 new ones are all `ar` |
| `layout-sweep` still defaults to `pages: ['overview']` | 1 occurrence in `tools/layout-sweep.mjs` | **no** — left as P56 left it. Every sweep in this phase passed all twelve explicitly |
| `contrast-sweep` and `verify-arabic` could not see a shell overlay at all | both scope to routed pages / `main` | **yes** — `--drawer 1` and `--page assistant` |
| `ProviderFaultsPanel` rendered the **same message key** as both its eyebrow and its `<h3>` | 2 occurrences of `diagnostics.providerFaults.title` in a 27-line file | **yes** |
| **51 colour literals** in `feature-layout.css`, in a product whose rule is that everything derives from the token layer | `grep -cE '#[0-9a-fA-F]{3,6}' ` → 51 | **partly** — 8 fixed where they sat on surfaces this phase collapsed (`#7e563a #435d70 #222b31 #77838c #173128 #9bd4bd #1b3a31` and a gradient). **43 remain**, untouched and out of scope |
| `--ac-material-elevated`, a **fourth** neutral surface level, used 60 times | `grep -c 'var(--ac-material-elevated)'` → 60 | **yes** — token, class and `MaterialSurface` level deleted |
| `AppShell.handleGlobalKeydown` calls `target?.matches` on an `EventTarget` that need not be an `Element` | 1 occurrence, unchanged since P50 | **no** — not reachable from real input (`document.activeElement` is always an element); recorded, not fixed |


---

## What a user can now ask their machine that they could not before

Before this phase the product had an embedded model that had generated exactly
nothing, a wire that had been called "chat" in a commit message and was not one,
and — after P56 — an engine that could generate but nothing anywhere to type
into. A user could read what AetherCore had decided to tell them, and that was
the whole conversation. They can now press `Ctrl+/` on any screen and ask a
question in their own words about *this* machine, and get back either an answer
in which **every claim carries the chip that proves it**, or the product saying
plainly that it has not measured the thing and naming the scan that would, or a
declared fault with its reason — and nothing else, ever, because the ungrounded
paragraph is discarded before it reaches the screen rather than shown greyed out
or badged "low confidence". That constraint is not a limitation the feature works
around: an assistant that can only tell you things about your own computer that
it can prove is the only kind this product was ever allowed to ship, and it is
something no general chatbot is. The drawer opens on a count of the evidence it
holds rather than a tour of what it might do, so the first thing it says is true.
