# AetherCore - Award-Winning Spatial Design System

## 1. Brand Identity & Visual Language (2026 Standards)
Inspired by the 2026 design awards (Apple Design Awards, Awwwards), AetherCore moves beyond basic modernism into **Adaptive Spatial Design** and **Glassmorphism 2.0 (Liquid Glass)**. The interface feels less like a flat screen and more like a physical, breathable instrument that adapts to the user contextually.

- **Style:** Spatial computing elements, Liquid Glass, intelligent minimalism.
- **Vibe:** Invisible, anticipatory, deeply professional, and organically integrated.
- **Core Principle:** "Machine Experience" (MX) - The UI must feel like it learns and breathes with the user, providing functional motion and adaptive context.

## 2. Global Styling (Spatial Depth & Liquid Glass)
- **Corners (Radii) - Fluid Topologies:** 
  - Cards & Modals: `32px` (Ultra-smooth organic curvature)
  - Interactive Elements: `16px` (Inviting, tactile)
  - Micro-components (pills, tags): `999px` (Fully rounded)
- **Glassmorphism 2.0 & Spatial Depth:** 
  - **Adaptive Transparency:** Backgrounds use layered `backdrop-filter: blur(40px) saturate(150%)` to hint at the content behind them without distraction.
  - **3D Hierarchy:** Subtle, multi-layered shadows to separate elements physically (Z-axis). No harsh borders. Elements float naturally.
- **Borders:** "Liquid Edge" highlights — a 1px inner border using a soft, translucent white (`rgba(255,255,255,0.08)`) simulating light hitting the edge of glass.
- **Typography:** `Inter Variable` or `San Francisco Pro` with dynamic weights based on context. `JetBrains Mono` for technical data, but strictly restrained to data fields.

## 3. Color Palette — six roles, not a mood board

Superseded 2026-09-03, corrected 2026-09-03 (owner review round 2). AetherCore
has exactly six color roles. Every state color anywhere in the product must
be one of these six — nothing is invented per-component.

| Role | Dark (fg / solid) | Light (fg / solid) | Means | Never |
|---|---|---|---|---|
| **Interactive** | `#5C7CFA` solid | `#4C6EF5` solid | Something actionable right now — a button, a link, a focus ring. | Express a state, or appear as a gradient. |
| **Verified** | `#D9B94E` / `#C9A227` | `#A9820A` | Cryptographically checked — a signed manifest, a hash-pinned model, a signed licence. | Appear as a gradient, a background fill, or on anything not actually verified. Rare by design. |
| **Healthy** | `#69DB7C` / `#2B8A3E` | `#2F9E44` | A measurement is inside its expected range. | — |
| **Attention** | `#FFC078` / `#E67700` | `#C96500` | Worth a look, not urgent — drifting toward a threshold. | — |
| **Critical** | `#FF8787` / `#C92A2A` | `#C92A2A` | Outside safe range now, or a fault occurred. | — |
| **Denied by policy** | `#CA92D3` / `#AD4EBC` | `#773781` / `#AD4EBC` | The system refused an operation on purpose, per a named rule. | Fall into the critical/red family. A refusal is the product working as promised, not an error. |

Base surfaces are unchanged: void `#030508` (dark) / `#EDF0F5` (light),
glass panels at low-alpha white/black per theme — see the `.ae` custom
properties in `AetherCore.html` for the literal values (`--void`, `--glass`,
`--glass-2/3`, `--edge`, `--edge-strong`).

### Round 2 corrections (owner review)

Two things were wrong in the first pass of this section, both about the same
pair — interactive and denied, the two that must never be confused, were the
two sitting closest together on the wheel.

**1. Interactive vs. denied moved from 23° apart to 64° apart, and got a
second, non-colour signal.** The first pass put denied at `#7C6BC4` (~251°),
23° from interactive's ~228° — both blue-violet, the smallest hue gap in the
whole set, on exactly the pair where confusion is least acceptable. Denied
moved to `#AD4EBC`, hue ~292° (plum/magenta), landing inside the requested
285–300° band. That's option (a). Option (b), taken as well rather than
instead: denied now has a shape signature nothing else in the file uses —
a 1.5px **dashed** border (every other border in the product is solid) on an
8px-radius chip (every other chip/tag/pill in the product is 999px, fully
rounded), plus an icon (`block`) that was freed from a collision with the
"Halt High-Usage Processes" button (moved to `stop_circle`) so it now belongs
to denied exclusively. Colour, shape, and icon all have to agree before
something reads as a policy refusal.

**2. The interactive gradient is gone.** `#5C7CFA → #7C93FF` was still a
gradient encoding nothing — the brief's own list of tells for generic work
names decorative gradients specifically. Every button, toggle, checkbox fill,
meter, progress bar, and avatar background that used
`linear-gradient(…, var(--a1), var(--a2))` now uses solid `var(--a1)`. `--a2`
is removed from both themes; nothing references it. This also fully retires
the original bug (interactive's second stop being literal healthy-green) —
removing the gradient removes the class of bug, not just this instance of it.

### Colour-blind check — simulated, not estimated

Hue-angle reasoning in the first pass was a guess and was caught being wrong
on the pair that mattered most. This time: six solid swatches
(`#5C7CFA #C9A227 #2B8A3E #E67700 #C92A2A #AD4EBC`, one per role) rendered in
Chrome headless, screenshotted once with `Emulation.setEmulatedVisionDeficiency`
set to each of `protanopia` / `deuteranopia` / `tritanopia` (and once with
`none` as a control), pixel colour read back from the PNG, converted
sRGB → linear → CIE XYZ → CIE Lab, and compared pairwise as ΔE (Euclidean
distance in Lab — roughly, >10 is "clearly different colours to most
observers", <5 is a real risk of confusion). Full 15-pair table per condition
is in the brief-v3 session log; weakest pair per condition:

| Condition | Weakest pair | ΔE | Note |
|---|---|---|---|
| none (control) | verified vs. attention | 35.6 | Both comfortably distinct; nothing under 35. |
| protanopia | verified vs. attention | 13.1 | interactive vs. denied is 2nd-weakest at 20.6 — clearly separated, not a risk. |
| deuteranopia | verified vs. attention | **7.7** | The one genuine weak spot — see below. |
| tritanopia | interactive vs. healthy | 22.8 | interactive vs. denied is 10th of 15 at 65.3 — the round-1 worry is fully resolved here, the condition it was originally flagged under. |

**interactive vs. denied, the pair this round of fixes targeted, is never the
weakest pair under any of the four conditions** (ranks 2nd, 5th, 2nd, 10th)
after the hue move — confirmed by simulation, not just claimed.

**The real weak spot the simulation found: verified vs. attention under
deuteranopia, ΔE=7.7.** This wasn't the pair under review, but the check is
supposed to catch what's actually there, not just what was asked about.
Flagging rather than silently patching: mitigated today by the same
shape-not-colour principle as denied — verified never appears as a status
pill/badge/tag (attention's entire vocabulary); it's a rare, fixed
icon+label seal that only ever sits beside a licence, a model hash, or a
consent record, never in a row's tone position. The two can't occupy the
same visual slot, so the weak ΔE doesn't currently translate into a
real-world confusion. If verified or attention ever grows a second visual
form, re-run this check before shipping it.

Two more pairs worth recording, inherited from the original three-color
system and out of scope for this pass (owner approved verified/healthy/
attention/critical unchanged): critical vs. attention weakens under
tritanopia (40.7 → 22.9) and healthy vs. critical weakens under
deuteranopia (105.4 → 16.5) — the classic red/green confusion, mitigated the
way it already was, by different icons, different copy, and critical's
exclusive toast treatment.

A seventh, non-role tint (`#8FA6FF`, tone `"info"`) remains in the row/table
helpers for neutral reference rows that aren't claiming healthy/attention/
critical about anything. Never appears in interactive's own pattern
(buttons), so it doesn't violate "interactive never expresses a state" —
it's not a state at all.

### Note for the Svelte port

This artifact embeds four full static weights of IBM Plex Sans Arabic
(~400 KB total) for simplicity and because a single self-contained HTML file
has no build step to subset at. The production app already ships a 1.07 GB
model — every avoidable megabyte matters there. Subset the embedded face to
the glyphs actually used (Arabic block + the specific Latin/digit/punctuation
set the UI strings need) before shipping.

## 4. UI Screens & Adaptive Elements
All original interfaces retained, but reimagined with predictive AI structuring and functional motion (e.g., Dynamic Island style notifications).

### 4.1. Overview (Adaptive Dashboard)
- **Elements:** 3D System Health Orb (breathes based on load), Minimalist sparklines for CPU/RAM.
- **Buttons:** `[Run Smart Scan]`, `[One-Click Optimize]` (Features spring-physics on click).

### 4.2. Deep Scan Results
- **Elements:** Fluid lists categorizing Junk, Privacy, and Registry. Smooth transition animations when expanding details.
- **Buttons:** `[Resolve All]`, `[Dismiss]`, `[Inspect Details]`

### 4.3. Hardware Health
- **Elements:** Spatial thermal maps (CPU/GPU), Battery lifespan curves, Disk SMART diagnostics.
- **Buttons:** `[Run Diagnostics]`, `[Export Spatial Report]`

### 4.4. Performance Telemetry
- **Elements:** Cinematic, real-time wave graphs for CPU/Memory/Network.
- **Buttons:** `[Halt High-Usage Processes]`, `[Engage Gaming Mode]`

### 4.5. AI Insights (Conversational UI)
- **Elements:** Natural language suggestions predicting next actions before the user asks.
- **Buttons:** `[Apply AI Strategy]`, `[Voice Query]`

### 4.6. Drivers Inventory
- **Elements:** At-a-glance driver ecosystem visualization.
- **Buttons:** `[Synchronize Drivers]`, `[Snapshot Current]`, `[Revert]`

### 4.7. Deep Clean
- **Elements:** Visual "dust" clearing animation representing freed space. Cache, Temp, Registry keys.
- **Buttons:** `[Initiate Deep Clean]`, `[Select All]`, `[Clear]`

### 4.8. System Repair
- **Elements:** System OS integrity nodes.
- **Buttons:** `[Heal OS Files]`, `[Reset Network Stack]`, `[Repair Registry]`

### 4.9. Recovery & Rollback
- **Elements:** A spatial timeline (scrollable Z-axis) showing historical system states.
- **Buttons:** `[Establish Restore Point]`, `[Time-Travel to Selected]`, `[Purge Old States]`

### 4.10. Crash History
- **Elements:** Minimalist timeline of BSODs with instant cause-identification tags.
- **Buttons:** `[Analyze Dump]`, `[Clear Timeline]`

### 4.11. Fleet Management
- **Elements:** Floating 3D network map of connected devices.
- **Buttons:** `[Provision Device]`, `[Broadcast Policy]`, `[Disconnect]`

### 4.12. Settings
- **Elements:** Haptic toggles (vibrate on click), Adaptive Theme, Notifications.
- **Buttons:** `[Commit Preferences]`, `[Factory Reset]`, `[License Portal]`

### 4.13. Command Palette (Omnibar)
- **Elements:** Center-screen, highly blurred spotlight search (like Raycast/macOS Spotlight).
- **Buttons:** `[Execute]`, `[Clear]`

### 4.14. Plan Review Consent
- **Elements:** Highly transparent, layered summary card focusing entirely on the action.
- **Buttons:** `[Authorize & Execute]`, `[Decline]`

## 5. Next-Gen Experience (MX) Rules
- **Functional Motion:** Every interaction (hover, click, load) must have meaningful, spring-based micro-animations. No static jumps.
- **Predictive UX:** Anticipate the user's need. If the CPU is hot, the "Optimize" button glows contextually.
- **Accessibility as Standard:** Dynamic type scaling, VoiceOver support, and high-contrast modes are built directly into the spatial layers.
