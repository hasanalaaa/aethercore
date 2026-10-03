# P87-02B — same-byte signed RC qualification

Scope: ASTRA §11 P87-02B and accepted D26; D0 adds no product wire fields. H1–H6 remain unchanged. This implements qualification/promotion eligibility, not publishing or a GA declaration. Real signed qualification remains blocked by the external prerequisites below.

## Source flow

`release.yml` first queries the existing production-signing environment with the Actions read token. Missing required reviewers, restricted deployment refs, a denied API or an absent environment fails closed. No new credential or provisioning mechanism is introduced. The signer job retains the exact isolated Windows/x64/aethercore-signing labels and checks actual runner context and a valid code-signing certificate with a non-exportable private key. It never exports key material.

The existing locked build and freeze verification run before packaging. `build-release.ps1` captures each payload/MSI/Burn unsigned hash immediately before its existing signing call. The final receipt binds the source SHA, both source seals, approved lock/freeze inputs, exact unsigned→signed transitions, the final release inventory and the pinned signer. The test-only care smoke client is built before acceptance, included under `acceptance/` and hash-bound by that inventory; it is not installed as product payload.

The separate acceptance job downloads the immutable RC produced by that signing job. It verifies the trusted job-output receipt and bundle hashes, exact current sealed source/freeze inputs, all package hashes and native Authenticode signatures before any installation. A previous signed baseline is downloaded by explicit run/artifact/source/bundle/provenance pins and must be an older version. No MSI tables or release bytes are rewritten to invent a baseline.

Only a dedicated disposable Windows 11 acceptance lane runs the existing lifecycle. The installed interval hook calls the six-symptom EN/AR probe while the service exists. Existing disposable-machine, elevation, existing-service, ACL, mutation-lock, installed-signature and uninstall guards remain. Installed executable hashes must match the RC payload. The signed upgrade installs the previous approved bundle then this same RC, checks installed security/bytes, and preserves sentinel bytes through update and uninstall.

The acceptance gate requires every lifecycle and update step to pass, both locales, a Windows 11 workstation, a real unelevated ordinary-user SID/token observation, all six required runtime symptom dispositions and raw witness hashes. Failed, blocked, skipped, not-supported, absent and hosted Windows Server evidence are not passes. The promotion job downloads the same RC and acceptance artifact and rechecks every hash/signature and evidence commitment. It emits a promotion-eligible RC artifact without rebuilding or invoking signing; `ga` stays false. Channel publication remains a separately authorized release operation.

## Qualification and rejection controls

Portable fixtures execute the actual verifier and promotion functions against miniature committed/sealed source trees and package files. Only the native signature provider is replaced in positive portable fixtures; there is no CLI bypass. Four evidence controls were red before evidence validation: an omitted locale receipt, a failed lifecycle step, a skipped symptom, and Windows Server masquerading as Windows 11. Additional controls cover source SHA, signature/signer, package substitution after acceptance, receipt substitution, unsigned transition omissions, smoke-byte substitution, ordinary-user and raw witness evidence. Native fixtures also call the actual Authenticode API on an unsigned temporary artifact. These are source/fixture proofs, not signing or real-installed qualification.

A second red run exposed four missing runtime constraints: unavailable-provider evidence,
Care restart evidence, no-op explanation and read-only-owner exclusion. The gate now
requires explicit true runtime checks and `read_only=false`. A completed scheduled shell
is insufficient to release nested worker ownership: `worker_ownership_released=true`
must come from the producer after actual terminal/drain observation. Missing/false
ownership proof retains the isolated installed service, even if the task has exited.
The hook uses the explorer owner's InteractiveToken with RunLevel Limited and confines
witness write access to fresh test output; no credential or service ACL is changed.

## Source/fixture verification receipt (2026-10-02)

The portable verifier suite ran 29 cases: 28 passed on macOS and the native-only
Authenticode case was explicitly skipped. The Windows suite passed all 29, including
an actual unsigned-file Authenticode observation and rejection. The suite runs
from pwsh, matching the workflow launcher; the verifier reuses that existing host
to avoid incompatible inherited module paths in a nested PowerShell 5 process. The ordinary-task
fixture ran five controls: absent desktop, limited interactive token, uncertain task
start, actual completed failure and completed shell with an active nested worker.
All passed; mocked task/ACL cmdlets invoked no real scheduler, installer or service.
Native PowerShell parsed the changed build, lifecycle, installed-security and hook
fixture scripts successfully. These native receipts use the copied source files,
not a signed package or an installed acceptance claim.

Eight active Python security/GA/architecture/adversarial/gate-reader audits passed.
`static_validate.py` ran 351 checks and rejected only
`phase8_script_reference_integrity`: this isolated source deliberately references the
root-owned, not-yet-committed `scripts/p87-installed-acceptance.ps1`. YAML parsing was
unmeasured because optional PyYAML is absent locally. No assertion was removed or
weakened. This commit **requires pairing** with the real root-owned producer and a
repeat of static validation and both source seals on the exact combined head. It does
not claim the combined P87 source gate or real installed qualification complete.
The producer must honor the receipt schema, drain active assessment/Care workers,
and prove actual Care reconnect/service restart; missing native conditions stay blocked.
 Full final integration CI is root-owned. No signer, installer lifecycle or existing owner service was invoked by this implementation session.

## External prerequisites still blocked

Read-only GitHub metadata on 2026-10-02: production-signing exists but has no required-reviewer rules and no deployment-ref policy. Only WINDOWS_RUNNER is a repository variable; production-signing variable and secret names are empty. Registered Windows runners carry aether-win, not aethercore-signing. The owner's existing installed product is not a disposable acceptance lane.

Before signed qualification, the owner must supply an isolated signing runner with the exact label, a non-exportable valid code-signing certificate, approved review/ref protections, AETHERCORE_CODESIGN_THUMBPRINT, an HTTPS AETHERCORE_TIMESTAMP_URL and a protected enabled AETHERCORE_UPDATE_TRUST_PATH. No values were printed and no security setting was changed.

A dedicated disposable aethercore-acceptance-win11 runner and actual Windows 11 EN/AR ordinary-user runtime/accessibility evidence are required. The previous signed bundle requires immutable workflow run id, artifact name, source SHA, provenance SHA256 and bundle SHA256 (workflow dispatch inputs or approved production-signing variables). Missing signer, baseline or required evidence blocks release; unsigned CI artifacts cannot substitute. API availability is checked through GitHub's existing read token; no new administration credential is required.

The protection metadata uses the documented [environment endpoint](https://docs.github.com/en/rest/deployments/environments#get-an-environment). The signed gate uses native Get-AuthenticodeSignature for trust and pinned-certificate checks.

Owner Windows inventory receipt (2026-10-02T00:47:37Z): Windows 11 Pro build
26200, workstation ProductType 1, no code-signing certificates in either the
CurrentUser or LocalMachine My store. The raw JSON's SHA256 is
`c641a40951f4b2cc25301bfc60baa0071c19e894e3d952cdbb8186d45931ed65`.
This is a concrete signer provisioning blocker; the local report location is
not treated as a portable qualification artifact.

## Disposable Care restart source handoff

The lifecycle now accepts the producer's first expected partial exit only when it
explicitly reports released worker ownership and a pending Care restart. Before any
service operation the parent checks the signed source/bundle/locale identity, the
selected desktop SID, actual reconnect proof, a recorded Care UUID and a closed
desktop. Only the newly installed isolated AetherCoreMaintenance service may be
stopped/started; each actual status wait is bounded to 30 seconds. Verification
uses a second Limited InteractiveToken task, the same SID/output/RC and
`-VerifyCareRunId` containing that exact UUID. It does not relabel another case.
One 660-second observer deadline spans both tasks and restart for each locale.
An expired deadline schedules no further work; timeout or unknown worker ownership
retains the service.

The native fixture executes the actual parent functions with substituted scheduler
and service-controller objects: eleven controls cover the original task cases,
same-run restart and its exact encoded argument, an unowned service, active nested
work, a changed desktop, missing disposable acknowledgement and an expired budget.
The flow was red before the restart parent existed, then green. The 29-case native
verifier suite and all eight active Python audits remain green. No real service was
restarted. Integration must pair the root-owned producer's VerifyCareRunId mode and
the real care_smoke reconnect/persisted-run helper (source handoff 4e1d549); the
combined source seal/static gate and real signed installed qualification remain
root-owned. Missing actual unavailable-provider or other runtime evidence still
blocks promotion.

## Paired source integration (2026-10-03)

The integration now includes the actual installed producer, its actual-function
controls and `care_smoke` exact-run reconnect/persistence helper. See
`P87-INSTALLED-ACCEPTANCE.md` for the flow and ownership limits. The earlier
missing-producer static-reference rejection above is a historical isolated-lane
receipt, not a waived gate. Current integration repeats static validation and
both source seals with the paired tracked files. Receipt validation additionally
requires `ok=true`, no blocked reason, a closed desktop, no pending restart, a
valid Care UUID and repair progress. Each required screen binds distinct typed
runtime/accessibility/screenshot witnesses and its actual installed selector.
Full native CI and real signed installed qualification remain separate gates.
