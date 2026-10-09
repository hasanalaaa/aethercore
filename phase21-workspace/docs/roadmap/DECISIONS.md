# Owner decisions on the architect's roadmap (D0–D34)

Source: [`ASTRA-PLAN.md`](ASTRA-PLAN.md) §12, "owner decisions grouped", and the decision text in the
task sections it cites. Each row is one decision: its ID, what it decides, the recommendation the
plan makes, and the date it was accepted.

**Provenance.** The owner accepted every recommendation in that table on 2026-09-28, when the planning
session presented them. The P77 execution session recorded this on 2026-09-29 from the planning
session's report, which quoted the owner's words: "أوافق على توصيات Astra كلها". This file records the
acceptance; it does not add to it.

**Rules that follow.** A recommendation that says "not now", "deferred" or "out of the first release"
stays out. Where the plan's default before a decision is "do not implement the blocked part", the
accepted recommendation is what unblocks it, and nothing beyond the recommendation.

| ID | Decides | Recommendation adopted | Date |
|---|---|---|---|
| D0 | Any wire change, even an optional field, needs owner acceptance before it is built | Narrow additive changes only; new tags after checking the latest contract; no renumbering; no undocumented JSON carried in technical strings; bindings regenerated through the repository mechanism, never by editing generated code | 2026-09-28 |
| D1 | Fleet/desktop error contract (only if P77-04B changes the desktop contract or a wire field is missing) | Reuse `common.proto:ErrorInfo` first; then add a narrow typed code | 2026-09-28 |
| D2 | Default online behaviour in driver search (touches the network) | Remove the default online path now and add no replacement online path here; the official WUA access after an explicit install approval is unchanged | 2026-09-28 |
| D3 | Cancel verbs carrying an assessment or plan ID | Yes. `CancelRequest` in `events.proto` keeps cancelling an RPC, not a long-running operation, and is not reinterpreted | 2026-09-28 |
| D4 | Any change to the consent policy | Do not change it | 2026-09-28 |
| D5 | A separate `PrepareCarePreview` verb | Yes: it reads and prepares only, and returns each domain's eligibility reason and the scan time; Auto is not widened to repair, drivers or disabling services | 2026-09-28 |
| D6 | Typed additive messages for ThermalZone, Battery, Boot and Network domains and their coverage (source, observed time or window, availability or reason key); NVMe `spareThreshold` fields when needed | Yes; no general plugin framework and no generic arbitrary JSON bag | 2026-09-28 |
| D7 | A new vendor sensor table, or a dependency or signed sensor provider | Deferred until devices, references, licences and tests exist; not added now. Out of the first release | 2026-09-28 |
| D8 | A launcher that affects reboot | Guided manual only | 2026-09-28 |
| D9 | A DbgEng dependency or symbol download | Deferred: no bundled debugger and no automatic symbol server. Out of the first release | 2026-09-28 |
| D10 | Dependency wiring between startup and crash, or new impact fields | Pass a typed snapshot from the existing composition instead of coupling the crates | 2026-09-28 |
| D11 | Independent DNS, TCP or HTTP connectivity tests | Outside the first execution: no internet probing in a local scan; at most an optional, explained step after consent with a declared destination, volume and timeout | 2026-09-28 |
| D12 | ETW and performance capture | Read-only Event Log and PDH stay the default; a local 60-second capture with explicit consent may be offered if a performance problem stays unexplained; no new dependency without D0 and the seal/freeze rule. Out of the first release | 2026-09-28 |
| D13 | A `ProblemStatus` or presence field, if it needs the wire | Additive in `drivers.proto`, as a separate contract task of at most 150 lines before it is shown; no inventory restructuring meanwhile | 2026-09-28 |
| D14 | Search scope or consent intent on the driver search request | Local is the default, even for the legacy empty request; online only after a confirmation that names Windows Update or the managed server; the scope enum alone is not owner authentication: the router binds it to the owner and the specific intent | 2026-09-28 |
| D15 | A path that allows an intentional downgrade or changes consent | Outside Auto, behind a separate consent. WinVerifyTrust succeeds only on 0; offline cache-only trust fetches no CRL; missing revocation stays unknown | 2026-09-28 |
| D16 | Any change to a safety tier or a bypass | Rejected | 2026-09-28 |
| D17 | Automatic rollback or a Catalog package importer | Later phases with a separate contract and consent each; now WUA execution plus manual recovery | 2026-09-28 |
| D18 | The repair source (touches the network, needs consent) | Local-only restore; an online repair source is a future option needing separate explicit consent, size, destination and validity of intent, and is never merged into Care | 2026-09-28 |
| D19 | Wording when cancellation is unsupported | "Stop at the first safe point"; no force termination under the name of safe cancel | 2026-09-28 |
| D20 | Extra repair actions, or changed consent or safety | Keep the existing list and fix eligibility first | 2026-09-28 |
| D21 | Destructive cleanup eligibility or retention | No new categories before these controls; changing destructive eligibility or retention needs acceptance | 2026-09-28 |
| D22 | The assistant's guarantee about facts | Fact-template outputs for facts, instead of an unprovable promise about free prose; if IDs need the wire, D0 first | 2026-09-28 |
| D23 | Locale on list requests | An optional field per D0, instead of guessing the text language or sharing a cache between languages | 2026-09-28 |
| D24 | Required branch checks (set outside the YAML) | A stably named aggregate check | 2026-09-28 |
| D25 | A Windows 11 VM and permission for install, uninstall and repair tests | An isolated environment owned by the project; no service expansion | 2026-09-28 |
| D26 | Signer, channels and promotion permissions | Do not declare GA before a signed, installed gate passes | 2026-09-28 |
| D27 | A wire coverage indicator, if needed | A small compatible addition after reviewing tags; no contract redesign | 2026-09-28 |
| D28 | A pagination cursor | Additive cursor with a fallback for old clients; never silently change the meaning of the integer cursor; pin the owner's snapshot while paging; reject expired, tampered or cross-owner cursors with a local message and a declared reload; no unbounded map without TTL and cap if a token is chosen | 2026-09-28 |
| D29 | Extending the measurement contract for timing | A small compatible addition if the contract needs it | 2026-09-28 |
| D30 | A change to when maintenance is available | Do not relax the barrier before Windows evidence | 2026-09-28 |
| D31 | Devices, VM images, certificate, media, and approval of specific destructive tests | Release claims limited to the tested matrix; the deliberate pipe DACL is not changed and no debt is resolved by weakening it | 2026-09-28 |
| D32 | Code signing for the first release (supersedes the "signed" part of D26) | The first release ships **unsigned**: no Authenticode certificate is bought or used, and SmartScreen / "unknown publisher" warnings are accepted. Gates and receipts say "unsigned by owner decision D32" and never claim signed. An unsigned RC is promotable only with this row present and the same-byte hashes verified; every other gate is unchanged, and no gate declares GA | 2026-10-04 |
| D33 | Where installed acceptance runs (supersedes the VM of D25 for this purpose) | On the owner's own Windows 11 PC, not a disposable VM. The owner accepts that the lifecycle installs, repairs, uninstalls and upgrades the AetherCore they use there. The run needs this row, an explicit `-OwnerHostAccepted`, a protected dated backup of the owner's AetherCore data before anything is touched, and the service running again at the end | 2026-10-04 |
| D34 | The repair-assessment symptom on the owner host (D33), where no provider is unavailable | It passes on real progress plus a declared terminal state, both observed on the installed RC. The unavailable-provider observation is recorded as **not observed on the owner host by owner decision D34** and is never claimed: the check stays false, and acceptance and promotion list it under `not_observed`. It does not apply to a disposable VM. No provider is broken on purpose and no time budget is widened; every other gate is unchanged | 2026-10-09 |

**D32 and D33 provenance.** The planning session relayed the owner's words on 2026-10-04:
"ما اريد اي شهاده للبرنامج لانها غاليه خلي تولي" (D32) and "ميحتاج VM خلي كلشي على الجهاز و النضام
الخاص به اعرف كل المخاطر و موافق عليهه" (D33). The owner then confirmed both directly to the executing
session the same day, answering «أؤكد القرارين» to the question that described each one.

**D34 provenance.** D33 run 6 (2026-10-09) passed five of the six symptoms on the owner PC. The sixth needs an
assessment that finishes while a provider is unavailable. That PC is healthy: WinRE is enabled and System
Restore has restore points. A full assessment (DISM, SFC, disk) also outlasts the probe's budget, so the probe
cancels it at the first progress. The executing session described this, recommended D34, and the owner
answered it directly: «أوافق على D34».
