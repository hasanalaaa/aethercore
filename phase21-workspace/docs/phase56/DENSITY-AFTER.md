# P56 — the Overview, after

Same instrument, same width, same definitions as [DENSITY.md](DENSITY.md).
Raw: `measure/after-populated.json`, `measure/after-empty.json`.

## The four numbers the direction set, and what they measured

| target (DIRECTION.md) | before | after | met |
|---|---:|---:|:--:|
| ≤ 6 distinct type sizes, all tokens | **15** (8 written outside the scale) | **6**, every one a token | ✔ |
| 3 neutral surface levels | **6** | **3** | ✔ |
| ≤ 30 prose words populated | 63 (en) / 52 (ar) | **0** / **0** | ✔ |
| ≤ 45 prose words empty | 167 (en) / 133 (ar) | **24** / **16** | ✔ |
| ≤ 1.5 words per reading, empty | 5.22 (en) / 4.16 (ar) | **0.75** / **0.50** | ✔ |

## Populated, 1280

| | before | after |
|---|---:|---:|
| prose words (en / ar) | 63 / 52 | **0 / 0** |
| prose runs | 7 / 6 | 0 / 0 |
| data readings | 90 / 89 | 90 / 89 |
| labels | 34 / 36 | 37 / 38 |
| distinct type sizes | 15 | **6** |
| surface treatments | 20 | 18 |
| — of which **neutral** (structure) | **6** | **3** |
| — of which role-tinted (semantics) | 14 | 15 |
| words per reading | 0.70 / 0.58 | 0.00 / 0.00 |
| page height | 1,715 px | 1,578 px (en) · 1,654 px (ar) |

## Empty, 1280

| | before | after |
|---|---:|---:|
| prose words (en / ar) | 167 / 133 | **24 / 16** |
| prose runs | 9 / 7 | 2 / 1 |
| distinct type sizes | 16 | **6** |
| neutral surface levels | 5 | **3** |
| words per reading | 5.22 / 4.16 | **0.75 / 0.50** |

The 24 words that remain empty are **not on the Overview**. They are the shell's
disconnected card — "The desktop service is stopped or this page is running
outside AetherCore. Start the service and retry the connection." — which states a
fact the reader cannot get anywhere else and names the action that fixes it. It
passes principle 2 and it is shell chrome, so it stays until Part 3 reaches the
shell. The Overview's own prose, populated and empty, is **zero**.

Zero is not a target that was chased. It is what is left after deleting every
sentence that failed the test in principle 2, and nothing that failed it carried
a fact:

| deleted | words | where the fact went |
|---|---:|---|
| `overview.subtitle` — "A unified workspace for safe maintenance, hardware evidence and recovery…" | 16 | nowhere. It asserted nothing measurable |
| `overview.telemetryNoHistory` — "No sparklines: this build retains the latest sample, not a series…" | 20 | the telemetry section's meta now reads **`3 samples · 1,000 ms observation window`**. The fact was *how many samples exist*; it is now a number |
| `overview.orbEmptyBody` — "This screen reads the performance counters itself, every 5 seconds while it is open…" | 34 | the headroom section's meta now reads **`every 5 s`** / **`live · 5 s`**. The fact was the cadence |
| `overview.actionItemsEmptyBody` — "Action items appear once a scan has reported. Each carries the observation it rests on…" | 29 | "Nothing has reported yet." The rest described how the screen works, and the empty state already lists the three scans it is waiting on |
| `overview.serviceLogEmptyBody` — "Every line here is an event the service pushed, with its own sequence number…" | 21 | "No events yet." The log itself shows what a line is |
| `overview.noActiveCopy` — "Start from Drivers, Repair, Deep Clean, or Startup Manager. AetherCore freezes the reviewed plan before requesting administrator consent." | 18 | "Nothing is running." The second sentence is the persistent policy band's own text, one screen region away |
| `overview.eyebrow` — "OVERVIEW" above an `<h1>` reading "Your PC, quietly under control." | 1 | the `<h1>` now reads "Overview" |
| the service pill — "Engine 0.1.0-fixture" in the header | — | it was the **third** rendering of connection state on one screen, after the rail and the context bar |
| six action-item titles, sentences → labels | 12 | "Devices reporting a driver problem" → "Driver problems"; "Hardware events in the last 30 days" → "Hardware events", with the window moved into the technical meta line beside the provider, where it is still a fact and no longer a sentence |

## Where the six type sizes go

| px | token | what wears it |
|---:|---|---|
| 48 | `--ac-type-hero` **(new)** | the headroom figure. Exactly one per view, per the dataviz hero-figure rule (≥48px, same sans, never a display face) |
| 34 | `--ac-type-display` | the screen `<h1>` |
| 20 | `--ac-type-title` | the four telemetry readings, the active plan's kind, an empty channel's `—` |
| 15 | `--ac-type-headline` | section titles, action-item titles |
| 13.5 | `--ac-type-body` | labels, buttons, channel readings, empty-state bodies |
| 11 | `--ac-type-technical` / `--ac-type-kicker` | every mono token, evidence chips, tags, section meta, the log |

Eight of the fifteen "before" sizes were literals — 9.5, 10, 10.56, 13, 13.12, 14,
17, 27, 44 px — and several sat within half a pixel of each other. They are gone.

## What changed outside the Overview, and why

`design-tokens.css` gained **one** token (`--ac-type-hero`). Five shared files
had size literals replaced with tokens they should always have used — this is the
design system, and the scale cannot exist on one screen only:

| file | change | visible effect elsewhere |
|---|---|---|
| `EvidenceChip.svelte` | `0.59375rem` → `--ac-type-technical`; `--ac-glass-3` → `--ac-glass-2` | chips are 1.4px larger and share the control surface with `.secondary` — one fewer neutral level on every screen |
| `EmptyState.svelte` | body `--ac-type-callout` → `--ac-type-body`; channel value `1.1875rem` → `--ac-type-title` | empty states 1px larger, channel readings 0.8px larger |
| `PolicyBand.svelte` | `0.625rem` → `--ac-type-kicker` | the band's label is 1px larger. Its violet identity, dashed border and 8px radius are untouched |
| `PolicyDenied.svelte` | `0.59375rem` and `0.65625rem` → `--ac-type-technical` | same |
| `materials.css` | context kicker and page name onto the scale; `.disconnected-card` headings onto `--ac-type-title` | 1–3px |
| `feature-layout.css` | `.primary,.secondary` `0.875rem` → `--ac-type-body` | every button in the product is 0.5px smaller |

**No screen's markup or copy was changed except the Overview's.** The roll-out is
Part 3 and is gated on the owner approving this direction.

### One regression found and fixed, and what it says about the gates

Moving `.shell-context-copy strong` from `0.82rem/1.2` to `--ac-type-body`/1.2
clipped Arabic ascenders by 2px — the box is `overflow:hidden` for its ellipsis,
so the line box *is* the clip box. It appeared on **every screen in the product,
in Arabic, in both themes**: 72 of 144 sweep combinations.

It was found because this pass ran `layout-sweep` across all twelve screens.
**The sweep's own default is `pages: ['overview']`**, so every previous run in
this project's history measured one screen. Eleven screens have never been swept
before today. Fixed by giving the line 1.45 rather than 1.2.

The same widened run surfaced a false positive: `.sr-only` elements are clipped
*by design*, and the sweep reported 12 of them on the Activity screen as
findings. The instrument now skips boxes of 1px or less — recognised by shape, so
any spelling of the pattern is quiet.

## Gates, after

```
node tools/verify-numbers.mjs    27 distinct score-shaped numbers, each traced
node tools/verify-arabic.mjs     7/7 pass; 0 glyphs from a system fallback font
node tools/verify-tokens.mjs     PASS — 466 var() declarations, 0 unresolved
node tools/layout-sweep.mjs      144/144 populated · 144/144 empty
                                 (12 screens x 2 languages x 2 themes x 3 widths)
node tools/contrast-sweep.mjs    3,198 text nodes, 0 below 4.5:1, both themes
npx svelte-check                 0 errors, 0 warnings
pnpm build                       built in 703ms
```
