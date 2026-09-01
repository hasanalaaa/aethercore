# AetherCore Instrument

**This document is the whole specification. Do not blend it with any earlier
AetherCore design system.** A previous system for this product called itself
"AetherCore Laboratory" and specified *Technical Minimalism*, a *rigid 1px
hairline grid*, *strictly flat with no shadows or blurs*, and *0px sharp
corners*. The owner rejected that result. Every one of those rules is
**revoked**. If any instruction below appears to conflict with a remembered
earlier rule, the instruction below wins.

## Brand & Style

AetherCore is a local, air-gapped instrument that tells the truth about the
machine it runs on. It is not a security dashboard and not a marketing surface.
It reports what it measured, names what it did not measure, and refuses things
out loud.

The visual language is **calm instrumentation**. Confidence is expressed through
restraint, generous radii and quiet depth — not through hard edges. An
instrument you trust does not shout.

Every value below is taken from the shipping application's own token file, so
the mockups and the product speak one language.

## Shapes

Radii are generous and consistent. **Nothing is 0px. Nothing is 4px.**

- Cards, panels, tables, dialogs, empty states: **18px**
- Nested blocks inside a card: **14px**
- Buttons, inputs, selects, evidence chips: **10px**
- Status pills, filter chips, counts, avatars: **fully rounded, 999px**

Sharp corners are not part of this system. There is no "sharp structural grid"
that rounded elements sit inside — the containers are rounded too.

## Elevation & Depth

Depth is the primary structural device, and it **replaces** the hairline grid.
Panels are cards: a lifted surface, a soft shadow, and a 1px inset white
highlight along the top edge.

- `card`: `0 1px 0 rgba(255,255,255,.035) inset, 0 18px 42px rgba(0,0,0,.18)`
- `float`: `0 28px 76px rgba(0,0,0,.46), 0 1px 0 rgba(255,255,255,.09) inset`
- `focus`: `0 0 0 2px #0b1016, 0 0 0 4px rgba(142,197,245,.86)`

The inset highlight is what makes a surface read as *lifted* rather than merely
*lighter*. Do not omit it.

Glass: structural chrome (sidebar, top bar) `blur(28px) saturate(135%)`;
elevated panels `blur(34px)`; focused overlays `blur(42px)`.

Borders are white at low alpha — `rgba(255,255,255,0.075)` subtle, `0.115`
default, `0.18` strong — never a solid grey line. A border is a seam, not a cage.

**Forbidden:** a full-bleed 1px grid across a screen; panels butted together
with a 1px seam; `panel-gap: 1px`; borders as the primary means of separation.
Cards are separated by **16px of space**.

## Colors

Surfaces are deep, cool and calm, anchored on `#0b1016`.

```
background / surface        #0b1016      on-surface           #f7f9fb
surface-dim                 #080d12      on-surface-variant   #a4afb9
surface-container-lowest    #070b10      outline              #788590
surface-container-low       #0e141b      outline-variant      #1e262f
surface-container           #141c24      primary              #8ec5f5
surface-container-high      #19222b      on-primary           #06213a
surface-container-highest   #202b35      primary-container    #2b5a86
surface-bright              #2a3540      on-primary-container #d5eaff
secondary  #d6a15f   on-secondary  #3a2708   secondary-container #7a5a24
tertiary   #a99ae0   on-tertiary   #241a52   tertiary-container  #4a3e82
error      #ef9a9a   on-error      #3a0d0d   error-container     #6b2626
```

### Reserved meanings — invariants

Six hues carry meaning. They are never decorative, never substituted for one
another, never re-assigned.

| Meaning | Identity hex | `-on-dark` text partner |
|---|---|---|
| Interactive | `#5B8DEF` | `#8EC5F5` |
| Denied by policy | `#7C6BC4` | `#A99AE0` |
| Verified / signed | `#C9A227` | `#D6A15F` |
| Healthy | `#3E9E6B` | `#83D5B2` |
| Attention | `#D08A2E` | `#E8BD78` |
| Critical | `#C24D4D` | `#EF9A9A` |

The identity hex is used for fills, dots, bars and borders. The `-on-dark`
partner is used only where the same meaning must appear as **text** on a dark
surface and needs AA contrast. The meaning never changes — only the lightness.

Two rules that are easiest to break:

- **`Denied by policy` is never styled as an error.** The product refuses things
  deliberately; a refusal is a correct outcome, not a failure. No red, no
  warning triangle, no error copy. Violet is its own identity.
- **`Interactive` never expresses a state.** Affordance only — buttons, links,
  focus rings, active selection. Never a severity, never a result.

Gold is used *only* beside a seal or a signature-verification result, never as a
generic highlight. `tertiary` in this theme **is** the policy violet, so a
denial can never fall through to the error token; do not use it decoratively.

## Typography

- **Geist** for everything a person reads: headings, labels, navigation, body,
  buttons, empty-state copy. It must not read as hardware documentation.
- **IBM Plex Mono** — a real monospace — only for machine facts: numbers, file
  paths, hashes, PIDs, exit codes, durations, byte counts, UUIDs. Anything a
  user might copy. **Prose is never monospaced.** Do not substitute a
  proportional face for the mono role; column alignment is functional.
- `label-caps` sparingly, tracking `0.04em` — enough to separate, not enough to
  look like a specification sheet.
- Never apply letter-spacing to body text, and **never to non-Latin scripts**.
  Tracking breaks Arabic joining. Latin wordmarks may be tracked; Arabic never.
  Arabic prose must never be set in a monospace face.

Scale: `display-lg` 32/40 600 -0.02em · `display-sm` 22/30 600 -0.015em ·
`headline-sm` 16/24 600 · `body-md` 15/24 400 · `body-sm` 13/20 400 ·
`label-caps` 11/16 600 0.04em · `data-mono` IBM Plex Mono 13/20 450 ·
`data-mono-sm` IBM Plex Mono 11/16 400 0.02em.

## Layout & Spacing

4px base unit. Page margin **32px**. Gap between cards **16px**. Card padding
**20px**. Table cells 16px × 12px, row height **44px** — readable density, not
maximum density.

Prefer fewer, larger, well-filled cards over many small panels. **A card that
holds one sentence is a layout bug:** give it real content or fold it into a
neighbour. No region of a screen may hold a large expanse of empty surface.

## Honesty rules — these outrank every visual rule above

- **Never invent a number.** No score, index, grade, percentage or rating that
  is not a direct readout of something measured, and no fabricated precision.
  `Engine Integrity 94.2%` is forbidden. `CPU utilization 87.4%` is fine — it is
  a readout of a real quantity. The test is traceability, not the `%` sign.
- **Never write scare copy.** No predictions of loss, breach or damage.
  Forbidden: *"Imminent data loss risk detected."*
  Correct: *"3 reallocated sectors · read 2m ago."*
- **Empty is not zero.** Uncollected values show **"Not collected yet"** — never
  `0`, never `--%`, never a gauge parked at the left stop. Pair it with the
  action that would collect it.
- **Every displayed value is traceable** to a real measurement with a timestamp.

## Components

- **Buttons** 10px radius. Primary = solid interactive fill. Secondary =
  `surface-container-high` fill, subtle border. Tertiary = text only.
- **Cards** 18px radius, `surface-container` fill, `card` elevation, subtle
  border, 20px padding. Header = `headline-sm` plus optional right action.
- **Status pills** fully rounded; fill = the reserved hue at ~16% alpha, label =
  its `-on-dark` partner. Dot indicators are **circles, 8px — never squares**.
- **Evidence chips** 10px radius, `surface-container-high`, `data-mono-sm`.
- **Inputs** 10px radius, `surface-container-low` fill, subtle border, focus
  ring as specified. No glow.
- **Tables** 18px radius on the container, no outer grid. Header row
  `label-caps` on `surface-container-low`. Row separators subtle and
  **horizontal only — never vertical column rules**.
- **Empty states** a filled `surface-container-low` block at 14px radius —
  **never a dashed outline**, never an oversized dashed rectangle. One short
  honest line, one action, sized to its content.
- **Policy band** a 3px fully-rounded vertical bar in the policy violet on the
  leading edge of a governed container, with a violet pill label. Calm, never
  alarming.
