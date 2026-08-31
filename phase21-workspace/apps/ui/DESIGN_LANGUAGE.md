# AetherCore design language

**Stance:** evidence-first technical calm. AetherCore should feel like a trusted instrument: quiet, legible, and explicit about what it knows, what it cannot know, and what it will refuse to do.

## Design anchors

- **Visible proof:** every advisory claim exposes its source evidence in one step. A citation is a primary action, not metadata hidden in a tooltip.
- **Governed change:** diagnosis is visually distinct from mutation. Read-only surfaces use cool neutrals; consented operations use the positive action color and state their reversibility.
- **Honest capability:** unavailable, unsupported, and denied values are rendered as named states—not zeroes, fake percentages, or disabled-looking silence.

## Tokens

The canonical tokens live in `src/design-tokens.css`. They are deliberately semantic so light/dark and high-contrast modes can change without component rewrites.

| Concern | Rule |
| --- | --- |
| Colour | Midnight structural background, cyan evidence/accent, green completion, amber caution, red fault. Never use colour without a text/icon label. |
| Type | Segoe Variable (display/text) and Cascadia Mono (technical evidence). Body is 15px minimum; captions are reserved for supporting context. |
| Rhythm | Four-pixel base rhythm. Use `--ac-space-*`; page sections use 24–48px, controls use 40px minimum height. |
| Surface | Material layers are translucent but boundaries remain visible. One elevation step per interaction layer; no decorative glass blur. |
| Motion | 90–140ms state feedback and transform/opacity only. Respect `prefers-reduced-motion`; no progress animation may imply work that did not happen. |
| Direction | Layout uses logical properties (`inline`/`block`). Technical values and paths are isolated LTR; Arabic copy remains naturally RTL. |

## Required component states

Every feature surface and actionable primitive must make these states reachable and distinguishable:

1. **Idle** — the next safe action and current evidence are visible.
2. **Loading** — reserve the final layout, announce busy state, and describe the operation; do not invent completion or percentages.
3. **Empty** — explain why no records exist and offer the relevant next action, if one is safe.
4. **Error** — state what failed, preserve the user's data, and offer recovery or a support-safe technical reference.
5. **Denied by policy** — explain that the refusal is an intentional protection (policy, consent, capability, or permissions), not a transient fault. Never present a destructive retry loop.

Use `aria-live="polite"` for state changes, `role="alert"` only for errors requiring immediate attention, and keep every control keyboard reachable with a visible focus ring.

## Evidence interaction

Claims should display a short human-readable finding, a confidence/availability label, and a **View evidence** action. The action opens the existing evidence/citation surface without losing the user's place. Technical identifiers are rendered with `TechnicalText`; never expose a full path containing a username.

## RTL and themes

The same hierarchy is used in English and Arabic, but alignment follows reading direction. Use CSS logical properties instead of left/right overrides and test navigation order, focus order, truncation, dialogs, and citation drawers in both directions. Light mode must use the same semantic contrast targets as dark mode; it is not a colour inversion.

## Review gate

Before shipping a surface, verify: zero new Svelte errors, no hardcoded product copy (add EN and AR catalog keys), no fake telemetry, no secret/path leakage, 44px interaction targets, visible focus, reduced-motion behaviour, and all five states above.
