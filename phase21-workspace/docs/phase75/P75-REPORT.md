# P75 report

AetherCore's claim is that every statement it makes is measured and every insight cites its
evidence. P75 ran as parallel lanes (`AMBITION.md`, `LANE_CONTRACT.md`), merged one PR at a time
with `main` green at every merge sha, then ran the whole product for real, took an independent
review of PRs #25–#51, and fixed what both exposed. A statement below without the command or
run that produced it is labelled a belief.

## 1. What merged

Waves 1–2 before this session: #25–#40 (see `HANDOFF.md`). This session:

| PR | lane | merge sha | PR CI (head) | `main` CI at merge sha | ledger |
|---|---|---|---|---|---|
| #47 | ui-truth | `41ea21d` | `36273802164` | `36277040057` | `DBT-P75-047`…`052` |
| #48 | service-host | `9f46bd0` | `36289024493` (+ installer `36289024176`) | `36291484445` | `DBT-P60-003`, `DBT-P75-056` |
| #49 | update-trust-2 | `acec25b` | `36292776315` | `36295466077` | `DBT-P75-053`…`055` |
| #50 | gate-honesty | `fcd6e32` | `36295528292` | `36298220504` | `DBT-P65-003`, `DBT-P75-057`…`060` |
| #51 | seal-root | `71c557d` | `36298275279` | `36301007852` | `DBT-P60-002` |
| #52 | care-session-consent | `c211de2` | `36329601263` | `36333300218` | `DBT-P75-045`, `-061`, `-062`, `-081` |
| #53 | service-cli-trial | `ef767ef` | `36333563123` | `36337293077` | `DBT-P75-063`…`068`, `-077`, `-078`, `-080`, `-084`…`087` |
| #54 | insight-grounding | `f0f2d7b` | `36337342711` | ``36341317167`` | `DBT-P75-069`, `-079`, `-082` |
| #55 | ui-trial | ``f4fb820`` | `36341411343` | ``36345316974`` | `DBT-P75-070`…`075`, `-083` |
| this PR | windows-smoke | — | see §4 | — | `DBT-P75-076` |

#52's first CI run (`36301094516`) failed on a host-speed assertion in the real-model test on
the 2-vCPU runner (head `8184148`). It merged on a later head (`5689620`) whose run passed; the
assertion itself was corrected in #54 to state the contract — finish, or stop honestly before the
deadline (`-079`) — without widening any deadline.

Every merge followed the protocol: `git merge origin/main`, ledger rows moved from the lane
document, `source_seal.py --json` first (every failing path a lane file), reseal, a CI run green
whose head_sha is the merged head, squash-merge, and `main`'s push run green at the merge sha
before the next merge.

## 2. DBT-P75-045 — hardened session consent (owner decision)

Neither ledger option. What shipped (#52, `c211de2`):

1. The care plan digest commits to each step's domain-plan **content** digest, not its id.
   Red-before: two plans with one id and different content produced one care digest.
2. One owner approval authorizes exactly its Auto steps, for one run, bound to the owner
   principal, single-use, void after 120 s. Red-before: one approval ran the same plan twice.
3. The grant carries the digest the owner saw (P75 review #39): if the plan composed at grant
   time differs, the service refuses `DigestChanged` and approves nothing. Red-before on #52's
   earlier head: work added after the preview ran (`left: 2, right: 0`).
4. Before each step, the approval becomes a one-shot authorization at the domain's **unchanged**
   barrier (`consume_consent_and_transition`) only while the plan's owner, state and content
   digest match; otherwise that step is refused `DigestChanged` and the steps still at their
   approved bytes run. Red-before: `one_approval_runs_every_auto_step` →
   `authorization required`.
5. ReviewOnly steps are refused before any authorization and reported Skipped.
6. The broker's per-plan 120 s path is untouched outside care.

On the Windows runner, against the installed product: red on `main` + smoke
(`windows-installer.yml` `36289309971`: "operation engine: authorization required"), green with
the lane (`36289311454`: Cleanup `VerifiedByDomain`; a second start without a new approval
`AwaitingConsent`). The UI dialog lists exactly the automatic steps one click approves; aetherctl
makes the owner retype the digest and sends it; `care consent-grant` requires `--plan-digest`.

## 3. Trial run

### 3.1 UI (fixture build, every screen)
Checked: 12 screens (overview, deep scan, drivers, repair, cleanup, startup, performance,
hardware, crash, activity, fleet, settings) × EN/AR × dark/light × 1280/960/720 with the
repository's `tools/layout-sweep.mjs` (overflow, clipped text, control over prose): 144/144
before and after the fixes. A scratch CDP audit per screen × locale × theme: foreign-language
text, WCAG contrast of every text node, a Tab walk (56–59 stops per screen, a painted focus ring
on every one), console errors; every Overview button clicked with the IPC command it sent; the
disconnected (empty) state; the care dialog opened and driven by keyboard only (Enter opens it,
focus lands on Cancel with a visible ring).

Found and fixed (lane `ui-trial`, and `care-session-consent` for care):
- the layout fixture used values the service never emits and answered `{}` to Fleet, so Fleet
  threw ("reading 'length'") and the sweep measured fiction (`DBT-P75-070`);
- real wire values printed raw — English inside the Arabic UI — startup scope, driver match
  quality, 17 recommendation-reason codes, target-version source, lowerCamel repair facts,
  `insight.summary.observation`/`securityPosture`, Fleet cadence and last result, recovery
  severity `Amber`; the evidence count said "item(s)" and "2 عنصرًا" (`DBT-P75-071`, `-083`);
- a Partial deep scan with 0 findings headlined "Healthy ✓" (`DBT-P75-072`);
- unmeasured disk activity read as a number (`DBT-P75-073`);
- sidebar labels ellipsized, Arabic truncation eating device names, "Bugcheck0x0000001AMeasured"
  (`DBT-P75-074`); an empty triage section with no sentence; "Events 12480" (`DBT-P75-075`);
- the care dialog opened with no plan loaded, raw step kinds, Refresh jumping to Overview, a run
  from Overview landing nowhere (`DBT-P75-061`); "approve" with nothing due (`DBT-P75-062`).

Contrast: the only text node under WCAG AA was the service log's decorative cursor (`▊`,
2.15:1 dark, 2.37:1 light). Screenshots: session scratch `trial/sweep` (before) and
`trial/sweep2` (after); they are evidence of this run, not repository artifacts.

### 3.2 Service and CLI on macOS
The unix maintenance service (`--foreground`, unix-ipc build, debug and release) on this Mac;
every aetherctl verb in text and JSON (about, version, capabilities, engine-source,
telemetry-once, self-check, service detect/units, doctor, perf start/stop/snapshot/report,
timeline page/patterns, care status/start, insights list/explain, scan start/status/history,
fleet list, schedule list, optimize plan/status, `--lang ar`); and a scratch IPC client that
sent 36 of the 69 verbs the UI uses (every read and every scan; mutating verbs are exercised
on Windows below). Each answer was compared with the OS: `df`, `iostat`, `memory_pressure`,
`sysctl kern.memorystatus_level`.

Found and fixed (lane `service-cli-trial`): doctor failed on first use (`-063`); the e2e suite
was red on `main` (`-064`); `--lang ar` printed English labels (`-065`); macOS reported used
disk capacity as disk activity, which fired IO_SATURATION (high, root cause) on an idle disk
(`-066`); macOS memory load 98% where the kernel said 34% used (`-067`); IO_SATURATION cited
evidence under its own threshold (`-068`); an empty care plan asked for approval (`-062`, care
lane). Recorded open: ~16 s without IPC at macOS start, 11.5 s of it Metal init (`-078`).

### 3.3 Windows, installed product
`windows-installer.yml` `bundle-log-acl-probe`, extended (lane `windows-smoke`): after
installing the real bundle it gates the service reaching RUNNING, 14 read-only aetherctl verbs
through the installed aetherctl (`"ok":true`, exit 0), and One-Click Care end to end with
`crates/ipc/examples/care_smoke.exe` over the named pipe (cleanup scan → plan → preview → one
approval → run → a second start refused), then uninstalls as before. First run (`36289311454`):
service RUNNING; 13 of 14 verbs ok; `doctor` exit 5 (fixed by `-063`); care PASS. On final
`main` the smoke caught its own client sending no digest with the grant — the service refused it
`care.error.planChanged`, as `-081` requires (`36338076643`). Fixed client, final run
(`36341345870`): 14/14 verbs ok, Cleanup `VerifiedByDomain`, a grant for a plan never shown
refused, a second start `AwaitingConsent`, `GATE: pass` (`DBT-P75-076`). The installer
probe for #48 measured `sc stop` reading `STOP_PENDING` (`36289024176`). The private firewall
profile's local registry key was measured on the runner (probe `36329924609`).

### 3.4 Intelligence, real model
`cargo test -p aethercore-intelligence-core --release --test embedded_generation -- --nocapture`
on this Mac (Metal): insight generation 70 tokens in 2.7 s. Every citation resolved, but two of
three insights stated counts their cited evidence contradicts ("3 occurrences" citing a pattern
of 5). Fixed: a line survives only if every number it states is held by its cited evidence
(`DBT-P75-069`); after: 1 insight. Residual: the surviving line calls one failed Startup plan
"a recurring failure", which is not mechanically checkable. The assistant tests asserted the
Windows runner's speed (`-079`); the feature-off build did not compile (`-082`, review #36).

### 3.5 Independent review (PRs #25–#51)
`AUDIT/P75-REVIEW.md` (untracked, read-only review). All eight findings were reproduced before
being fixed; none was dropped. #51 seal fail-open with `.github/` absent (`-080`, test red:
`exit=0 ok=True`); #39 grant bound to a recomposed plan (`-081`, test red: `left: 2`); #36
`--no-default-features` build (`-082`, 4 compile errors; new CI check); #29 insight key in no
catalog (`-083`, test reads the keys from the Rust sources); #28 Everyone deny before an
Authenticated Users allow (`-084`); #28 private firewall profile path (`-085`, measured on the
runner); #33 telemetry owner race (`-086`, deterministic test seam); #49 an address ending a
sentence (`-087`).

## 4. Gates at the final tree (AMBITION §6)
Run on this lane's tree after merging `main` at `f4fb820` (the Rust gates at `00f210f`,
whose Rust and scripts are identical: #55 changed only `apps/ui`, `ci.yml` and docs), on this
Mac with `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`:

| command | result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test --workspace --locked` | exit 0 — 142 test binaries, 755 passed, 0 failed, 1 ignored |
| `python3 scripts/static_validate.py` | exit 0 — `"ok": true`, 348 checks, 0 failed, 1 unmeasured (`parse_yaml`: PyYAML is not installed here; CI parses the workflows) |
| `python3 scripts/test_gate_readers.py` | exit 0 — "all 14 readers fail closed" |
| `python3 scripts/ps_marker_scan.py` | exit 0 — 81 scripts, 321 assertions, 0 failed, **90 UNMEASURED** (inline `if (…) { throw }` conditions and expression arguments the scanner does not evaluate; listed in its output, none counted as passing) |
| UI unit tests (ci.yml's command) | exit 0 — 28 tests, 28 pass |
| `pnpm --dir apps/ui build` | exit 0 |
| `python3 scripts/source_seal.py --json` | `"ok": true` — 1545 of 1545 tracked files verified; `.github`: 7 of 7 |

CI: `ci.yml` on this PR at its final head, and `main` at its merge sha — ids in the PR and in
`HANDOFF.md`; `windows-installer.yml` `bundle-log-acl-probe` on this lane: `36341345870`
(`GATE: pass`). Not run here: the PowerShell enterprise gates (no `pwsh` on this Mac; they run
in CI's Windows job) and the Windows build (CI only).

## 5. Open, and why
- `DBT-P75-052` — insights and the assistant send no locale, so model prose reaches the Arabic
  UI in English: a wire-contract change for the owner.
- `DBT-P75-055` — the support-bundle privacy report has no counter of its own for hosts, IPs and
  MACs: a wire-contract change.
- `DBT-P75-077` — audits red on `main` with no CI caller (`phase18_1`, `phase19` P19-PLAN-002,
  `phase27` p27-wirefreeze, `phase35`, `phase28` p28-deps/sealed snapshot): their expected
  values are phase-scoped; retiring or re-baselining them is the owner's call.
- `DBT-P75-078` — macOS service reachable only after the model loads (~16 s).
- macOS/Linux fault rates and per-core arrays (Wave 3, providers parity); `fuzz` lane;
  `DBT-P74-002`; the `ASSISTANT_DEADLINE` and every timeout are unchanged.
- Cleanup the owner must approve: remote branches `probe/*`, `wip/*`, merged `lane/*`, and the
  session's scratch worktrees.
