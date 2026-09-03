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

Superseded 2026-09-03 (brief v3). The palette below replaces the "Aurora
Glow" gradient described in prior revisions of this file: that gradient ran
`#5C7CFA → #82C91E`, blue into green, which meant the primary-button accent
and the "healthy" status color were the same hue at one end. A button and a
measurement should never look like the same kind of fact. AetherCore has
exactly six color roles. Every state color anywhere in the product must be
one of these six — nothing is invented per-component.

| Role | Dark (fg / solid) | Light (fg / solid) | Means | Never |
|---|---|---|---|---|
| **Interactive** | `#5C7CFA → #7C93FF` | `#4C6EF5 → #6C8CFF` | Something actionable right now — a button, a link, a focus ring, the primary gradient. | Express a state. Never used to say a measurement is good, bad, or refused. |
| **Verified** | `#D9B94E` / `#C9A227` | `#A9820A` | Cryptographically checked — a signed manifest, a hash-pinned model, a signed licence. | Appear as a gradient, a background fill, or on anything not actually verified. Rare by design. |
| **Healthy** | `#69DB7C` / `#2B8A3E` | `#2F9E44` | A measurement is inside its expected range. | — |
| **Attention** | `#FFC078` / `#E67700` | `#C96500` | Worth a look, not urgent — drifting toward a threshold. | — |
| **Critical** | `#FF8787` / `#C92A2A` | `#C92A2A` | Outside safe range now, or a fault occurred. | — |
| **Denied by policy** | `#9C8FD9` / `#7C6BC4` | `#6B5CA5` / `#7C6BC4` | The system refused an operation on purpose, per a named rule. | Fall into the critical/red family. A refusal is the product working as promised, not an error. |

Base surfaces are unchanged: void `#030508` (dark) / `#EDF0F5` (light),
glass panels at low-alpha white/black per theme — see the `.ae` custom
properties in `AetherCore.html` for the literal values (`--void`, `--glass`,
`--glass-2/3`, `--edge`, `--edge-strong`).

**The fix that mattered:** the health orb's "healthy" fill used to be
`rgba(130,201,30,0.42)` — the literal RGB of the old `--a2` green, i.e. the
interactive accent's second gradient stop, standing in for a status. It now
reads `rgba(43,138,62,…)`, the actual healthy-solid RGB. Interactive's
second stop moved from green (`#82C91E`) to a second blue (`#7C93FF`) so
this collision can't recur elsewhere in the gradient.

**Colour-blind check.** Approximate hues: interactive ~230°, verified ~46°,
healthy ~130°, attention ~30°, critical ~0°, denied ~255°. Two pairs sit
close enough on the wheel to worry about:
- *attention (30°) vs. verified (46°)* — the closest pair on paper. Kept
  apart in practice because verified never appears in the tag/badge
  vocabulary attention uses — it's a rare, fixed icon+label seal, never a
  status pill on a row.
- *interactive (230°) vs. denied (255°)* — closest under a blue-yellow
  (tritanopia) simulation. Kept apart because denied never appears as a
  gradient or a button fill (interactive's exclusive pattern), and always
  carries a mono rule ID next to it that interactive never does.
- *critical (0°) vs. attention (30°)* — the classic red-green-deficiency
  confusion pair, inherited from the original palette. Mitigated the same
  way it already was: different icons, different copy ("critical" vs.
  "warning"), and critical alone gets the persistent toast treatment.

Net rule: every hue pair close enough to risk confusion is also separated
by a second channel — shape, rarity, or accompanying text — so no two roles
collapse to "the same thing" even in grayscale.

A seventh, non-role tint (`#8FA6FF`, tone `"info"`) remains in the row/table
helpers for neutral reference rows ("browser session artifacts", "startup
chain") that aren't claiming healthy/attention/critical about anything.
It's a lighter, desaturated tint of interactive's hue but never appears in
interactive's own pattern (buttons/gradients), so it doesn't violate the
"interactive never expresses a state" rule — it's not a state at all.

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
