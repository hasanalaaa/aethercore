# P75 lane `ui-truth` (Wave 2) — evidence (ledger `DBT-P75-047`…`052`)

Branch `lane/ui-truth`, from `main` `174a9e4`. No dependency or lockfile change.

Scope used: `apps/ui/**` (not the Wave 1 catalog keys), `apps/desktop/src/main.rs`, one check added
to `scripts/static_validate.py`, and one step added to `.github/workflows/ci.yml` (UI unit tests).

UI logic is tested with node's own test runner and type stripping
(`node --experimental-strip-types --test apps/ui/tests/*.test.ts`), so no test dependency was
added. Behaviour in the running UI was measured in the fixture build (`pnpm fixture`) with real
`KeyboardEvent`s.

## Rows

| row | was | now | red-before / measured |
|---|---|---|---|
| `DBT-P75-047` | every `Ctrl+Shift+<digit>` shortcut was dead: the handler matched `event.key` against `[0-9]`, and with Shift held a US layout reports `!` for Digit1. On an Arabic layout `Ctrl+Shift+F`, `Ctrl+K` and `Ctrl+/` were dead too (`ب`, `ن`, `ظ`) | `navigationShortcut` / `commandShortcut` read `event.code`; Ctrl+Alt (AltGr) is never a shortcut | old logic extracted verbatim: `navigation.test.ts` failed 3/5 then 2/3 (`actual: undefined`); fixture UI: `Ctrl+Shift+3` with key `#` opened Drivers, Arabic `Ctrl+K` opened the palette |
| `DBT-P75-048` | every page said "Denied by policy: Outbound network" — false: the update check and Fleet connect on the user's action | the label says what is refused: network use the user did not start (`اتصال بالشبكة لم تطلبه`); rule id `NET-NO-EGRESS` unchanged (service and support-bundle contract) | copy change; the no-egress lane is what made "at rest" true |
| `DBT-P75-049` | the 13 `fleet_*` Tauri commands were plain `#[command] fn`, run on Tauri 2's main thread; they open SSH sessions with 45 s timeouts, so the window froze | `#[command(async)]`: tauri-macros 2.6.3 runs a synchronous fn so marked on its thread pool (`sync_threadpool`); no signature or invoke change | `static_validate.py` `p75_fleet_commands_off_main_thread`: fails on the old `main.rs`, passes now; `cargo clippy -p aethercore-desktop --all-targets -D warnings` clean |
| `DBT-P75-050` | "Run due schedules" always reported success, including an unreadable schedule store, a failed trust store, or failed hosts; the schedule handlers let a rejected invoke escape, so a transport failure showed nothing | `runDueOutcome` counts failed hosts and collects errors; failure text `fleet.errScheduleRunDue` (EN, AR); every handler reports a rejection | `fleet.test.ts` against the old always-success logic: 4/4 fail |
| `DBT-P75-051` | the assistant drawer had no live region, so a screen-reader user never learned an answer had arrived; closing it left focus nowhere | a polite status region announces each settled turn once (streaming tokens are not announced); focus returns to the element that had it when the drawer opened | fixture UI: focus on the palette button → `Ctrl+/` → focus in `#assistant-input` → Escape → focus back on the palette button; the status region is present |
| `DBT-P75-052` (**OPEN**) | insights and the assistant send no locale, so model prose reaches the Arabic UI in English; backend `detail` strings (Fleet results, service errors) are shown raw | not changed: carrying the locale adds a field to `AskAssistantRequest` / `RequestInsightRequest`, a wire-contract change for the owner; raw details need a key per backend message | — |

## Not done

* `DBT-P75-052`, above.
* "a11y labels" from the Stage 0 list named no control; none was changed without a finding to
  reproduce.

## Local proof (macOS)

* `node --experimental-strip-types --test apps/ui/tests/navigation.test.ts apps/ui/tests/fleet.test.ts` → pass 12, fail 0
* `pnpm --dir apps/ui check` → 0 errors, 0 warnings (232 files)
* `cargo clippy -p aethercore-desktop --all-targets --locked -- -D warnings` → clean
* `static_validate.py` → `"failed": []`; `test_gate_readers.py` → all 14 fail closed;
  `ps_marker_scan.py` → assertions=234 failed=0 unmeasured=3; `test-phase12-localization.py` →
  34/34, EN/AR keys=1695; the audits naming touched files pass, except `phase27`, `phase34`,
  `phase35` and `sigma-master-full-app-ui`, whose verdicts are identical on `origin/main`.
