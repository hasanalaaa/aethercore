# `DBT-P61-001` — the 108, classified before anything is fixed

108 checks fail across six gates on the delivered tree. They are not one problem.
A **tree** failure is cured by changing the product; a **check** failure is cured
by changing the gate. Applying either cure to the other class does damage: fixing
a stale gate by editing the product invents work, and fixing a real defect by
editing the gate deletes the alarm. So this phase sorts them and fixes none.

Reproduce the population:

```
cd phase21-workspace && python3 scripts/classify_gate_failures.py
  FORMAT        47
  ABSENT        22
  MIXED          3
  UNCLASSIFIED  36
  TOTAL        108
```

`classify_gate_failures.py` runs each gate in-process with its own token helper
(`marker` in `static_validate.py`, `has` in the other five) wrapped, so every
verdict comes from the gate's own sources and the gate's own tokens. It does not
re-implement any check.

**Confirmed on Windows.** Run `34766915417` reached step 17 — the first run ever to get
there — and `Platform-neutral invariants` reported `checks: 347` and the same 28 failing
names, in the same order, as this Mac. The split below holds on the platform that
matters, name for name.

Measured first, before any of this: the 108 is the same 108 before and after
P62's `DBT-P61-002` commit — all nine gates run against a `git worktree` at
`0512b1d` and against the working tree, identical failure names in both. The
figure in P61's report is confirmed check-for-check.

---

## The finding: most of the 108 is one root cause, and it is not the product

**47 of 108 fail only because of whitespace.** These gates assert by substring,
and their tokens are written with the spaces removed — `hardware_gate:IsolationGate`,
`record.claimed=true`, `percent_delta>=5`, `if args.len()!=5`. The tree is
`rustfmt`-clean (`cargo fmt --check`, exit 0), so the source says
`hardware_gate: IsolationGate`. The construct the check is looking for is present;
the spelling predates the formatter.

Sampled to the line rather than inferred:

| check | token | where the tree actually has it |
|---|---|---|
| `phase13_provider_fanout` | `hardware_gate:IsolationGate` | `crates/diagnostic-engine/src/lib.rs:223` |
| `phase13_top_level_scan_panic_guard` | `catch_unwind(AssertUnwindSafe(\|\| run(worker_inner,owner)))` | `crates/diagnostic-engine/src/lib.rs:299` |
| `phase13_memory_unavailable_is_unknown` | `pub memory:Option<MemoryTelemetry>` | `crates/diagnostic-engine/src/lib.rs:121` |

**A further group fails for the same reason one step further on.** `rustfmt` does
not only insert spaces: when it wraps a signature or an enum across lines it also
adds a trailing comma, which removing whitespace cannot undo. Every one of these
was read by hand and the construct is present:

| check | token | the tree |
|---|---|---|
| `support_preview_precedes_prepare` | `prepare(&self,owner:&str,preview_id:&str)` | `crates/support-bundle/src/lib.rs:307`, wrapped, trailing comma |
| `support_strong_verifier_requires_independent_fingerprint` | `verify_archive(bytes:&[u8],expected_public_key_fingerprint_sha256:&str)` | `crates/support-bundle/src/lib.rs:853`, wrapped |
| `commit_fence_three_state` | `CommitFenceState { Active, Committed, Revoked }` | `crates/collector-runtime/src/lib.rs:211-216`, one variant per line |
| `provider_control_propagation` | `fn hardware(&self, control: CollectorControl)` | `crates/diagnostic-engine/src/lib.rs:163`, wrapped |
| `legacy_service_download_rpc_disabled` | the match arm and its error on one line | `services/maintenance-service/src/router.rs:774` and `:785` |
| `phase10_read_watcher_lease_release` | counts `else { break };` | the tree has `else { break; };` — 6 of them, and the check wants ≥ 5 |

None of these is a product defect. The cure for all of them is in the gate.

---

## The one cluster that looks like a lost safety property, and is not

Eight `ABSENT` checks across `zenith-recursive`, `enterprise-adversarial` and
`static_validate` name one coherent design in `crates/ipc`: `cancel_registered_io`,
`SyncIoCancellation`, `bind_reader_to_current_thread`, `OpenThread(THREAD_TERMINATE`.
None of it is in the tree. Read cold, that is eight missing cancellation and
teardown guarantees.

It is not. `git log -S` names the commit:

```
482d480  Windows IPC: convert named-pipe I/O to overlapped on both sides
  "Windows request/response has never worked. Both ends performed blocking I/O
   from two threads on one synchronous kernel file object ... every ServiceJob
   verb hung."
```

`482d480` is an ancestor of `main` (`git merge-base --is-ancestor`, exit 0). It is
the commit that both removed `cancel_registered_io` and introduced `CancelIoEx`.
The gates describe the *pre-overlapped* design; the product deliberately replaced
it to fix a bug that made the IPC unusable. The checks were never updated because
they have not run since P58.

**These are check failures.** Editing the tree to satisfy them would reintroduce
the defect `482d480` fixed.

Four neighbours that *look* like this cluster are not it, and were separated by
reading rather than by pattern:

| check | token | what the tree has |
|---|---|---|
| `ipc_trusted_service_sid_resolution_is_bounded_and_cached` | `TRUSTED_SERVICE_ACCOUNT` | `TRUSTED_SERVICE_SID`, `crates/ipc/src/windows_impl.rs:256` — renamed |
| `machine_mutation_authority_is_installer_provisioned_and_acl_hardened` | `SERVICE_PRINCIPAL`, `{SERVICE_PRINCIPAL}:F` | a local `principal`, `apps/install-hardener/src/main.rs:155` — renamed |
| `ipc_has_dedicated_client_writer_thread` / `..._server_writer_thread` | `write_client_frame(&mut writer_file` | `write_client_frame(&mut writer_io`, `windows_impl.rs:1002` — renamed |

The second is worth stating outright because `DBT-P61-002` rests on it:
`ensure_mutation_lock_file` (`apps/install-hardener/src/main.rs:162`),
`MACHINE_MUTATION_LOCK_RELATIVE_PATH`, `*S-1-5-18:F` and `*S-1-5-32-544:F` are all
present. **The machine-mutation lock really is installer-provisioned and
ACL-hardened**; that check fails on an identifier name, and P61's description of
the lock is confirmed rather than undermined.

---

## What is genuinely the tree's

Six, all in `static_validate.py`, each with the command that shows it:

| check | what is actually wrong | evidence |
|---|---|---|
| `phase8_cargo_deny_policy` | `deny.toml` has `bans.wildcards = "warn"`, not `deny`, and sets none of `sources.unknown-registry`, `sources.unknown-git`, `sources.required-git-spec` | `tomllib` read of `deny.toml` |
| `parse_json` | a UTF-8 BOM in `docs/phase36/evidence/*.json` — 4 files `json.loads` cannot parse | the gate's own `errors` list |
| `phase11_all_buttons_tactile` | 10 of 80 `<button>` tags carry no `use:fluidPress`; 8 are in `features/fleet/FleetPage.svelte`, a surface added after the Phase 11 invariant was written | regex over `apps/ui/src/**/*.svelte` |
| `phase10_service_decomposed` | `main.rs` is 397 lines against a `< 220` budget; `router.rs` is 1,868 against `< 240` | `wc -l` |
| `phase11_material_hierarchy` | `--ac-material-elevated` is defined nowhere under `apps/ui`; `base`, `focused` and `structural` are. Only `--ac-blur-elevated` exists | grep over every `.css` in `apps/ui` |
| `phase7_mica_desktop_shell` | `noRedirectionBitmap` is absent from the window config — `transparent`, `mica` and `fluentOverlay` are all set | `tauri.conf.json` |

Two of these are worth more than their check count. `phase8_cargo_deny_policy` is a
supply-chain policy that is simply not switched on, in a repository whose whole
release story is a dependency freeze. `phase11_all_buttons_tactile` is a real,
small, product regression concentrated in one file.

`phase10_service_decomposed` is listed as the tree's but is honestly a judgement:
the budgets are a P10 architectural decision and the router has grown 7.8× past it.
Whether the cure is the router or the budget is a decision, not a measurement, and
this document does not make it.

---

## Scope failures — the check is right about the invariant and wrong about where it looks

| check | why it fails | why it is not a tree failure |
|---|---|---|
| `path_dependencies` | `path = "../operation-kernel"` inside `PHASE_20_BINARY_SAFE_PATCH/new-files/**` | those trees are patch payloads, deliberately not resolvable where they sit; the walker descends into them |
| `phase7_all_eight_surfaces` | asserts `shortcut count == 9` | navigation now has 16 surfaces and 12 `Ctrl+Shift+` shortcuts; all nine required ids are present. The equality, not the product, is stale |
| `phase4_ui`, `phase5_ui_passive_default`, `phase6_ui` | literal screen copy: `DEEP CLEANUP`, `HARDWARE TELEMETRY`, `STARTUP & BACKGROUND SERVICES` | the UI was localized. The copy now lives in `apps/ui/src/lib/i18n/catalog.en.ts` and was rewritten (`'cleanup.title': 'Deep Clean'`). The one token in this group that carries a *safety* meaning rather than a label — "No logged memory hardware errors ≠ RAM proven healthy" — survives as `'hardware.truthNote'` |

---

## Two that this document does not resolve

`ipc_pipe_create_instance_regression_test` and
`ipc_production_pipe_owner_and_server_authority_are_service_scoped` both assert the
production pipe security descriptor is

```
O:{service_sid}D:P(A;;GA;;;{service_sid})(A;;0x00120003;;;AU)
```

and `crates/ipc/src/windows_impl.rs:286` builds

```
O:{service_sid}D:P(A;;GA;;;{service_sid})(A;;FR;;;AU)(A;;0x00000002;;;AU)
```

The authenticated-users ACE changed. It is not a rename and not whitespace:
`FILE_GENERIC_READ` (`0x120089`) plus `FILE_WRITE_DATA` (`0x2`) is not
`0x00120003`. Both checks' other tokens match, so the tree still declares
`EXPECTED_CLIENT_ACCESS: u32 = 0x0012_0003` and still asserts
`PIPE_CLIENT_ACCESS_MASK` against it — the constant and the descriptor now
disagree with each other.

Whether the split ACE is a deliberate narrowing or a defect is an access-mask
question, not a grep, and it is the one row in the 108 that could be a live
security difference. **Unresolved, and counted as unread below rather than as a
check failure.**

## The residue

36 checks fail on a predicate that is not a token search — a count, a line budget,
a parse, an ordering — and the tool reports them `UNCLASSIFIED` rather than
guessing. 17 of those are in `static_validate.py` and were read by hand for this
document; the verdicts above cover them. **19 remain unread**, in
`phase13-reliability`, `phase14-scheduler`, `phase15-security`,
`zenith-recursive` and `enterprise-adversarial`. They are named by
`classify_gate_failures.py` and are not classified here. The two pipe-descriptor
checks above are counted with them, making **21 unread**.

## The count

| | checks | failed | tree | check | unread |
|---|---|---|---|---|---|
| `static_validation` | 347 | 28 | **6** | 22 | 0 |
| `phase15_security` | 106 | 29 | 0 | 24 | 5 |
| `zenith_recursive` | 120 | 26 | 0 | 17 | 9 |
| `enterprise_adversarial` | 88 | 14 | 0 | 11 | 3 |
| `phase13_reliability` | 61 | 6 | 0 | 5 | 1 |
| `phase14_scheduler` | 63 | 5 | 0 | 2 | 3 |
| | **785** | **108** | **6** | **81** | **21** |

`zenith_adversarial` (35) and `phase16_policy` (42) fail nothing.
`phase12-localization` exits 1 without emitting a `checks` object and is not in
this population; that is its own row to open.

The order the cures should be applied in, which is the order the Windows job will
meet them: the 81 check failures are one edit to the gates' comparison — normalize
whitespace, and re-read the handful the formatter also re-punctuated — and the 6
tree failures are six separate small decisions, of which `deny.toml` is the only
one that touches a security posture.
