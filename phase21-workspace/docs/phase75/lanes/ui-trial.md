# P75 lane `ui-trial` (part 3, trial run) — evidence

Found by running `apps/ui` in the fixture build and measuring every screen: 12 pages × EN/AR ×
dark/light at 1280/960/720 (`tools/layout-sweep.mjs`: overflow, clipping, overlap — 144/144
before and after), plus a scratch audit (headless Chrome over CDP) for foreign-language text,
WCAG contrast of every text node, a Tab walk of 56–59 stops per page checking a painted focus
indicator (0 stops without one), console errors, and a click of every Overview button with the
invoke it sends. Screenshots: session scratch `trial/sweep`, `trial/sweep2`.

| row | commit | what was wrong | now | red-before |
|---|---|---|---|---|
| `DBT-P75-070` | `4bc81b9` | the layout fixture used values no service emits (`Registry`, `Measured`, `Informational`, `Enabled`, `insight.storageLatency`) and answered `{}` to `fleet_*` (FleetPage threw "reading 'length'") | wire-true values; Fleet answered with the desktop's shapes | audit: Fleet console errors 1 → 0 |
| `DBT-P75-071` | `f7b9a55` | real wire values printed raw: startup scope Machine/User, matchQuality Vendor family/Device class/Unknown, 17 RecommendationReason codes, targetVersionSource, lowerCamel FactState, "item(s)" plural, `insight.summary.observation`/`securityPosture` keys, Fleet cadence `every_hours:N` and English last-result, recovery severity `Amber` | labels in EN and AR | `tests/wire-values.test.ts` against `c5b2fa0`: 9/9 fail |
| `DBT-P75-072` | `b5b0254` | a Partial deep scan with 0 findings (5 collectors unavailable, measured on this Mac) was headlined "Healthy ✓"; unknown status fell to Healthy | "No findings in what could be checked"; unknown reads Unknown | new module test |
| `DBT-P75-073` | `0d76a3d` | disk activity the provider did not measure read as a number | em dash when the snapshot carries `storage.activeTime` | test: pct 0 before |
| `DBT-P75-074` | `9cc9918`, `1f515dd` | sidebar labels ellipsized ("Use dark t…", "استخدام الس…"); Arabic truncation ate a device name's beginning; the bugcheck cell read "Bugcheck0x0000001AMeasured" | wrap; LTR device names right-aligned; grid | sweep 144/144; fixture screenshots |
| `DBT-P75-075` | `6c02719` | Crash triage showed a heading over nothing; "Events 12480" unformatted | empty-state sentence; formatted count | fixture |

Contrast: the only text node under WCAG AA was the service log's decorative cursor glyph (`▊`,
2.15:1 dark / 2.37:1 light), aria-hidden decoration, not text. Focus: every Tab stop painted a ring.

Local proof: `node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts` 23/23; `pnpm check` 0/0; `test-phase12-localization.py` 34/34 (EN/AR keys 1743); `static_validate.py` failed [].
