# AetherCore — Design System of Record

**Status:** authoritative. Supersedes the pinned `ROUND_FOUR` / "AetherCore Laboratory"
system that the owner rejected on 2026-09-01.

- **Stitch project:** `17123055019148305822` — "AetherCore Professional System Instrument"
- **Design system asset:** `assets/4ef4cca5178f4d38be4428b96335d569`
- **System name:** `AetherCore Instrument` (was `AetherCore Laboratory`)
- **Ground truth for tokens:** `phase21-workspace/apps/ui/src/design-tokens.css`

This file exists because two previous sessions produced a `DESIGN.md` and a
`REVIEW.md` that were never written to disk. Everything below is committed.

---

## 1. Why the previous system was rejected

The design read as sharp-cornered 1990s boxes. The cause was not the screens —
it was the pinned design system, which mandated the look explicitly:

| Pinned rule (old `designMd`) | Consequence |
|---|---|
| `roundness: ROUND_FOUR` | 4px corners everywhere |
| "All UI elements use **0px (Sharp)** corners." | sharp boxes |
| "This system is strictly **Flat**. There are no shadows or blurs." | no depth |
| "1px `Hairline` borders are the **only** method used to separate areas" | cage-like grid |
| "panels are butted against each other with a 1px gap" | `panel-gap: 1px` |
| `IBM_PLEX_SANS` for headline + body | reads as hardware documentation |

The screens were obeying the system. Fixing screens without fixing the system
would have regressed on the next `apply_design_system`.

**The Stitch system was the outlier, not the app.** The shipping Svelte app
already uses a soft language. Verified in
`phase21-workspace/apps/ui/src/design-tokens.css`:

```css
--ac-radius-xs: 0.4375rem;  --ac-radius-sm: 0.625rem;  --ac-radius-md: 0.875rem;
--ac-radius-lg: 1.125rem;   --ac-radius-xl: 1.5rem;    --ac-radius-pill: 999px;

--ac-shadow-card:  0 1px 0 rgba(255,255,255,.035) inset, 0 18px 42px rgba(0,0,0,.18);
--ac-shadow-float: 0 28px 76px rgba(0,0,0,.46), 0 1px 0 rgba(255,255,255,.09) inset;
--ac-shadow-focus: 0 0 0 2px var(--ac-bg-base), 0 0 0 4px rgba(142,197,245,.86);

--ac-blur-structural: 28px;  --ac-blur-elevated: 34px;  --ac-blur-focused: 42px;
--ac-saturation: 135%;
```

The new Stitch system is derived from these values rather than invented, so the
mockups and the product now speak one language.

---

## 2. What changed

| | Before | After |
|---|---|---|
| roundness | `ROUND_FOUR` | `ROUND_TWELVE` |
| headline / body / label font | `IBM_PLEX_SANS` | `GEIST` |
| mono | IBM Plex Mono (used for labels too) | IBM Plex Mono, **data only** |
| colorVariant | `FIDELITY` | `TONAL_SPOT` |
| neutral seed | *(unset)* | `#0b1016` (app `--ac-bg-base`) |
| tertiary | `#ffb960` amber | `#7c6bc4` policy violet |
| panel-gap | `1px` | `16px` |
| grid-margin | `16px` | `32px` |
| body-md | 14px / 20px | 15px / 24px |
| label-caps | 700 wt, `0.06em` | 600 wt, `0.04em` |
| structure | 1px hairline grid, flat | elevation + soft shadow + blur |
| corners | "Nothing is rounded" | "Nothing is 0px" |

`colorMode` stays `DARK`.

### Radius scale (mapped to the app)

| Token | Value | App equivalent |
|---|---|---|
| control — buttons, inputs, chips | 10px | `--ac-radius-sm` |
| inner — nested blocks | 14px | `--ac-radius-md` |
| card — panels, tables, dialogs | 18px | `--ac-radius-lg` |
| chip — pills, counts, avatars | 999px | `--ac-radius-pill` |

---

## 3. What the new system deliberately keeps

The rejection was about *shape and voice*, not about *values*. These carry over
unchanged and are non-negotiable:

- **Dark mode.** This is an instrument, not a document reader.
- **Monospace for machine facts.** Numbers, paths, hashes, PIDs, exit codes,
  durations, byte counts. Column alignment is a functional requirement, not a
  style. What changed is that mono no longer leaks into prose and labels.
- **Information density.** Row height went 32–36px → 44px. Readable density,
  still dense. This is not a consumer dashboard.
- **The honesty invariants** (§5). They outrank every visual rule here.
- **The six reserved colour meanings** (§4), byte-for-byte.
- **The policy band** — governed containers still get a leading violet bar.
  It is now 3px and rounded rather than 4px and square.

---

## 4. Colour — reserved meanings are invariants

Six hues carry meaning. They are never decorative, never substituted for one
another, never re-assigned.

| Meaning | Invariant hex | `-on-dark` partner | App token |
|---|---|---|---|
| Interactive | `#5B8DEF` | `#8EC5F5` | `--ac-shadow-focus` ring |
| Denied by policy | `#7C6BC4` | `#A99AE0` | *(none — see §7)* |
| Verified / signed | `#C9A227` | `#D6A15F` | `--ac-accent` |
| Healthy | `#3E9E6B` | `#83D5B2` | `--ac-positive` |
| Attention | `#D08A2E` | `#E8BD78` | `--ac-warning` |
| Critical | `#C24D4D` | `#EF9A9A` | `--ac-danger` |

The invariant hex is the identity — fills, dots, bars, borders. The `-on-dark`
partner is used only where the same meaning must appear as **text** on a dark
surface and needs AA contrast. `#7C6BC4` on `#0b1016` measures ≈4.1:1, below
the 4.5:1 body-text threshold, which is why the partner exists.
**The meaning never changes — only the lightness.**

This two-value pattern is not an invention; it is what the shipping app already
does (`#3E9E6B` → `--ac-positive #83d5b2`, and so on).

### The two rules that are easiest to break

- **`DENIED BY POLICY` is never styled as an error.** The product refuses things
  deliberately. A refusal is a correct outcome, not a failure. No red, no
  warning triangle, no error copy. It has its own violet identity.
- **`Interactive` never expresses a state.** It is affordance only — buttons,
  links, focus rings, active selection. Never a severity, never a result.

Gold is used *only* beside a seal or a signature-verification result. It is
never a generic highlight.

### Seed colour — reconsidered and kept

The seed stays `#5B8DEF`. Reason: every other hue in the palette is *spoken
for* — green, amber, red, gold and violet each carry a fixed meaning. Blue is
the only hue left that carries none, so it is the only safe hue to derive a
whole tonal system from. A seed that carried meaning would spray that meaning
across every surface tint in the product.

What changed instead is the **neutral**: `overrideNeutralColor` is now the app's
own `#0b1016`, and `colorVariant` moved `FIDELITY` → `TONAL_SPOT`. `FIDELITY`
pulls surfaces hard toward the seed, which made everything faintly blue-washed
and let the interactive colour read as atmosphere. Now the surfaces are calm
cool slate and the blue means *"you can touch this."*

---

## 5. Honesty invariants (outrank every visual rule)

- **Never invent a number.** No score, index, grade, percentage or rating that
  is not a direct readout of something measured. No fabricated precision.
- **Never write scare copy.** No predictions of loss, breach or damage. State
  what was observed and when.
  - forbidden: *"Imminent data loss risk detected"*
  - correct: *"3 reallocated sectors · read 2m ago"*
- **Empty is not zero.** Uncollected values show **"Not collected yet"** — never
  `0`, never `--%`, never a gauge parked at the left stop. Pair with the action
  that would collect it.
- **Every displayed value is traceable** to a real measurement with a timestamp.

A percentage is *not* automatically a violation. `CPU utilization 87.4%` is a
readout of a real quantity. `Engine Integrity 94.2%` is a composite of nothing.
The test is traceability, not the presence of a `%`.

---

## 6. Structure — depth replaces the grid

- Panels are **cards**: lifted surface + soft shadow + a 1px inset white
  highlight on the top edge. The inset highlight is what makes a surface read as
  *lifted* rather than merely *lighter* — do not omit it.
- Borders are white at low alpha (`0.075` / `0.115` / `0.18`), never a solid
  grey line. A border is a seam, not a cage.
- Never a full-bleed 1px grid. Never panels butted together with a 1px seam.
- Glass: structural chrome `blur(28px) saturate(135%)`, elevated panels
  `blur(34px)`, focused overlays `blur(42px)`.
- **A card that holds one sentence is a layout bug.** Give it real content or
  fold it into a neighbour.
- **Empty states are filled blocks at 14px radius — never dashed outlines**, and
  sized to their content.

---

## 7. Known gaps and limits — recorded, not fixed

1. **Stitch emits zero `@media` queries.** Measured across all 16 screens:
   `0` occurrences of `@media` in 349,477 bytes of generated HTML. **No claim
   about responsive behaviour may be made from Stitch output.** Responsiveness
   is owned by the Svelte app and verified there.
2. **Arabic must not be judged from a Stitch screenshot.** The Arabic screen's
   root is `<html class="dark" dir="rtl" lang="ar">` and the file contains
   exactly one `tracking-widest`, on the Latin `<h2>AetherCore</h2>`. There is
   **zero** letter-spacing on Arabic. Disconnected glyphs in a Stitch screenshot
   are Stitch's renderer lacking an Arabic font. Arabic is handled in code.
   *(A previous session reported "letter-spacing breaks Arabic joining" and
   "mirrored LTR layout" from a screenshot. Both were wrong.)*
3. **`policy-denied #7C6BC4` has no token in the shipping app.**
   `design-tokens.css` defines accent/positive/warning/danger but no policy
   colour. The invariant exists in the design system only. Real gap.
4. **Arabic subtitle uses `font-data-mono-sm`.** Monospace applied to Arabic
   prose (`التشخيص والصيانة`) violates §4's "prose is never monospaced" and most
   mono faces have no Arabic coverage. Not yet fixed.
5. **One screen is a light-mode variant.** `542dadd6…` "Overview (Light)".
   The Stitch design system has a single `colorMode`, so applying the DARK
   system to it will flip it dark. There is no way to hold both in one system.

---

## 8. Screen inventory

16 UI screens, plus one logo image asset (`a560f6e5…`, no HTML) which is why
the project appears to have 17 screen instances.

| Screen ID | Title |
|---|---|
| `3db2c0d3a92746e981fbca2938af8f3a` | Overview (Dark) |
| `542dadd66f7948b3996645d2ea5fda6b` | Overview (Light) |
| `0fde03fc8dbe40298a34f4c3650fb624` | Overview (Arabic RTL) |
| `2264e37c37054c42bc6684fe018c2ec7` | Deep Scan Results (Dark) |
| `2ae0f56b71d347258b80996984998154` | Hardware Health (Dark) |
| `b89a8a21bf384376aba0f1cb7b7569b5` | Performance Telemetry (Dark) |
| `a5d3dfa2945b409eab5ca0418dd8cb3d` | AI Insights (Dark) |
| `212befabec664ebbbc398c9868c80d11` | Drivers Inventory (Dark) |
| `dc664f6439dc4af4a69ef49627de1dd8` | Deep Clean (Dark) |
| `bb657c826c41450fa3a0668f9dd6f018` | System Repair (Dark) |
| `91719bb7ed934739a4193a72370e5cd3` | Recovery & Rollback (Dark) |
| `a6b8d566a0de4d9db768def7826fd02b` | Crash History (Dark) |
| `fd63c0e395f14b08a675f25dd140bebd` | Fleet Management (Dark) |
| `f9e095c57a2741a783515f6618fc7c35` | Settings (Dark) |
| `e7adfbce623a4fc2b8ca33c15f378889` | Command Palette (Dark) |
| `a77e2b1091c344499aa2db3525016543` | Plan Review Consent (Dark) |

Baseline HTML as it stood *before* this session: `design/stitch/baseline-before/`.

---

## 9. Tooling findings — measured this session

These are raw observations from this session's runs. They change how the Stitch
project must be driven, so they are recorded here rather than in a chat log.

### 9.1 `apply_design_system` rewrote the design system it was applying

Applying the original asset `4ef4cca5…` did not apply it. A server-side agent
(`generatedBy: polish_edit_theme_agent`) **regenerated** the system and pushed it
back toward the rejected doctrine:

| Field | Sent | Came back |
|---|---|---|
| `displayName` | `AetherCore Instrument` | `AetherCore Laboratory` |
| `roundness` | `ROUND_TWELVE` | `ROUND_EIGHT` |
| `data-mono` fontFamily | `IBM Plex Mono` | `Geist` — monospace lost |
| `panel-gap` | `16px` | `1px` |
| `grid-margin` | `32px` | `16px` |
| designMd | elevation + generous radii | "rigid hairline grid", "primarily Flat" |

The asset carries a **`styleGuidelines`** field holding the old *"Technical
Minimalism… rejects depth metaphors, shadows, and rounded corners"* text.
`update_design_system`'s schema has **no field to write it**, so it stayed stale
and the polish agent re-derived `designMd` from it.

**Workaround, verified:** `upload_design_md` + `create_design_system_from_design_md`
produces a *new* asset (`43773111ef7b4a7cbc10cc4c322270cb`) whose
`styleGuidelines` is generated from the uploaded spec. Re-applying that asset
left it intact — `version` stayed `1`, monospace survived, spacing survived.
The clean spec lives at `design/stitch/DESIGN_SYSTEM.md`.

Consequence: **never `update_design_system` an asset that has legacy
`styleGuidelines`.** Create a fresh asset from a spec file instead.

### 9.2 `apply_design_system` does not change shape, elevation or structure

Measured on Overview, before vs after applying the corrected system:

| Measure | Before | After |
|---|---|---|
| `borderRadius` config | DEFAULT 4px · lg 8px · xl 12px | **identical** |
| `rounded*` class occurrences | 1 | **1** |
| `shadow` occurrences | 0 | **0** |
| `backdrop-blur` occurrences | 0 | **0** |
| `dashed` occurrences | 1 | **1** |
| bytes | 19,427 | 17,702 |

Fonts and colours **did** change (Geist + IBM Plex Mono loaded, surface
`#0f141a`). Shape, depth and the dashed empty state did **not**.

`ROUND_TWELVE` / `ROUND_EIGHT` had no effect on the emitted Tailwind
`borderRadius` scale, which stayed at the 4/8/12px default in both runs.

**So the roundness enum and the design system alone cannot fix the "sharp boxes"
complaint.** Shape lives in each screen's own markup. Screens must be changed
with `edit_screens`.

### 9.3 The original screen prompts bake in the rejected look

Overview's stored generation prompt contains, verbatim:

> "Uses hairline grid and monospace data."

So the hairline grid has **two** sources — the design system (now fixed) and the
per-screen prompt (still present). Any `edit_screens` call must explicitly revoke
it, or a later regeneration will reintroduce it.

### 9.4 `apply_design_system` replaces a screen with a new ID

Applying to instance `3db2c0d3…` produced `24a8318790ac4b48b9f66a24ed8cf016`,
still titled "Overview (Dark)". The old ID is **gone** from `list_screens` — it
is a replacement, not a duplicate. Same for `a77e2b10…` →
`964039fd2db94a58bebb2cabe4a5b194` (Plan Review Consent).

Screen count is stable at 16 UI screens. But **every screen ID in §8 changes the
first time that screen is themed**, so §8 must be re-read from `list_screens`
rather than trusted after any apply. `upload_design_md` also adds a
non-UI "DESIGN.md" screen (`14024205626593851266`) to the project canvas.

### 9.5 `edit_screens` timed out

Consistent with the 2026-09-01 note that `generate_screen_from_text` and
`generate_variants` timed out repeatedly. The tool documents that the operation
may still complete server-side after the client times out, and that it must not
be retried. Outcome recorded separately below once verified.

---

## 10. Results — what actually landed

`apply_design_system` run against the corrected asset
`43773111ef7b4a7cbc10cc4c322270cb` across all 16 UI screens. All returned
`status: COMPLETE`, and the design system survived every apply intact
(`version` stayed `1`, `data-mono` stayed IBM Plex Mono, `gap-card` stayed 16px).

### Landed ✅

| Check | Result |
|---|---|
| Geist loaded | **16 / 16 screens** |
| IBM Plex Mono retained for data | 9 / 16 (every screen that has machine data) |
| Surfaces recoloured to the calm slate ramp | all 16 |
| Design system integrity across 4 applies | held — no regression to "Laboratory" |
| Honesty violations | **1 / 16 screens** (Deep Scan Results only) |
| Arabic root + tracking | `<html dir="rtl" lang="ar">`, one `tracking-widest`, on Latin "AetherCore" |

### Did NOT land ❌ — the shape complaint is still open

Measured across all 16 themed files:

| Measure | Total across 16 screens |
|---|---|
| emitted `borderRadius` scale | `DEFAULT 4px · lg 8px · xl 12px` — **identical on every screen** |
| `rounded*` class uses | 52 (≈3 per screen; **5 screens have zero**) |
| `box-shadow` / `shadow-*` | **5** |
| `backdrop-blur` | **5** |
| `border-dashed` | **2** (Overview, Performance Telemetry) |
| `@media` | **0** |

The system specifies 18px cards, 14px nested, 10px controls and a soft card
shadow. **None of it reached the CSS.** The theme path controls colour and type
only. The owner's "sharp-cornered 1990s boxes" complaint is therefore **not
closed** by the system change alone.

### Blocked

`edit_screens` — the only path that can change shape — **timed out twice**:
once on `GEMINI_3_1_PRO` with a long prompt, once on `GEMINI_3_FLASH` with a
short one. Neither landed; the Overview HTML file ID `b83c98815db74dda…` was
unchanged after both. This matches the 2026-09-01 note that
`generate_screen_from_text` and `generate_variants` also timed out repeatedly.
The read path and the theme path work; **the generate/edit path does not.**

Consequently **Step 4** (Overview shape + empty state) and **Step 5** (Deep Scan
honesty violations) could not be executed. They remain open, with exact prompts
ready in this session's history.

### Honesty violations still present

| Screen | Violation | Status |
|---|---|---|
| Deep Scan Results | `Engine Integrity 94.2%` — composite of nothing, fabricated precision | open, needs `edit_screens` |
| Deep Scan Results | "Imminent", "data loss", "risk detected" — scare copy | open, needs `edit_screens` |
| Performance Telemetry | `85% impact`, `42% impact` under "AI Bottleneck Attribution" — unitless derived score | open, **not in the briefed scope**, needs a decision |

Not violations, checked and cleared: `CPU utilization 87.4%`, `Memory usage
99.1%`, `Context Window Utilization 84%`, core `Utilization 68%`, and the raw
`smartctl` block (`Available Spare 8%`, `Percentage Used 98%`). Each is a direct
readout of a real quantity. The test is traceability, not the `%` sign.

### Minor, recorded not fixed

`IBM+Plex+Sans` is still being fetched by 9 / 16 screens even though Geist is
now the specified face. Dead weight in the `<link>`, not necessarily applied.
