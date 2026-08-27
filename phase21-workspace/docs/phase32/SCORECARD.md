# Phase 32 — Scorecard

Phase: SECURITY AUDIT & WORKSPACE HYGIENE (read-only, CIS-mapped)
Host: macOS 26.6.2 (unix proof host) · toolchain 1.97.1 · workspace `phase21-workspace`

## Task completion

| Task | Status | Evidence |
|---|---|---|
| Baseline gates (verbatim, pre-change) | ✅ | P31 audit checks=727 failures=[] PASS (after H3 .DS_Store sweep); cargo test ×2 = 435 passed / 0 failed both runs; verify_phase31 --full-tree 979/979 PASS |
| H1 warm cache | ✅ | Full cold rebuild performed during baseline runs; session stayed warm |
| H2 superseded-archive archival | ✅ | P26..P29 zips → `_archive/` (8,752,668,501 bytes relocated); SHA-256 pinned pre-move in HYGIENE.md; KEEP set untouched; seal-extraction fallback added openly |
| H3 stray artifacts | ✅ | 1 root `.DS_Store` removed (hash recorded); zero `(1)` duplicates / `__MACOSX` in tree |
| S1 crate skeleton | ✅ | `crates/security-audit` — AuditTarget enum (7 variants), SecFinding+try_new guard, EvidenceRef, Severity/Confidence orderable, report_digest deterministic, hard-bound clamp family |
| S2 providers | ✅ | sshd lint (6 rules, verbatim citations), password policy (+macOS typed NotAvailable), sudoers parse-only (includedir expansion, never invokes sudo), filesystem (world-writable/SUID/.ssh/bounded walk), authlog bursts (sliding window, deterministic), firewall config-presence only, secrets scanner (4 detectors, first-4-char redaction contract) |
| S3 CVE air-gapped | ✅ | vulndb.json (36 curated NVD-derived entries) + sha256 manifest pin + entry-count pin; fail-closed loader; read-only census (pkgutil receipts/Homebrew Cellar/dpkg/rpm-presence); join engine w/ version comparator; network banned at symbol+dep+CLI layers |
| S4 service/wire/CLI | ✅ | Request tag 90, Response tag 52, EventKind 29, EventEnvelope tag 38 (all freeze lists extended, nothing renumbered); router arm w/ shared traversal guard + honest NotAvailable propagation + SecurityAudit event publish; aetherctl `sec audit/report`, `compliance summary --profile cis-l1`, `vulndb update --from/--dest` (--url rejected typed); Insights SecurityFinding surface + RuleFallback rule 4 |
| S5 compliance scaffolding | ✅ | assets/vulndb/cis_map.json (25 rule codes mapped or explicitly unmapped-with-note); format gate ^CIS L[12] \d+\.\d+(\.\d+)?$ enforced in crate tests AND audit script |
| S6 GD proofs | ✅ | GD-1 weak/hardened fixtures exact findings + verbatim line cites; GD-2 planted key found/redacted (`AKIA****************`) + raw-leak assert + clean-dir zero false positives; GD-3 seeded join exact set + unknown package ignored; GD-4 LIVE real-host audit digest stable ×2 (`98de3d20…487576`); GD-5 LIVE single-byte flip → fail-closed HashMismatch, lane degrades honestly, restore verified |
| S7 adversarial audit | ✅ | scripts/phase32-adversarial-audit.py — strict superset importing P31 in-process; principled named-list superseded filters (wire re-freeze bounds; offline writer scoped like export.rs); new gates a–j; total **checks=817** (>727) |
| S8 docs | ✅ | ARCHITECTURE.md (rule catalog + trust model + privacy contract + air-gap mechanics), QUALIFICATION_DEBT_APPEND.md (QD-032-001..003 appended to register, append-only), ISSUES.json (5 issues incl. self-caught fs::write violation), SCORECARD.md, MASTER_DELIVERY_REPORT.md, HYGIENE.md |

## Acceptance gates

| Gate | Result |
|---|---|
| GA baseline unchanged pre-change / post-change full-tree verify / delta sets equal | ✅ (see MASTER_DELIVERY_REPORT tails) |
| GB cargo test ×2 identical greens | ✅ exact counts in MASTER_DELIVERY_REPORT |
| GC phase32 adversarial audit PASS | ✅ checks=817, failures=[] |
| GD five live proofs green | ✅ incl. real-host digest ×2 |
| GE svelte-check 0 errors / warnings unchanged at 17 | ✅ `svelte-check found 0 errors and 17 warnings in 3 files` |
| GF clippy clean all-targets | ✅ exit 0; **zero clippy hits in any P32-authored file** (pre-existing legacy lints in untouched files unchanged, documented in MDR) |
| GG hash-manifest patch + EXPECTED_FULL_SHA256 + independent reverify ×2 | ✅ (tails in MASTER_DELIVERY_REPORT) |
| GH deterministic master archive ×2 + FINAL_SHA256 + sizes | ✅ (hashes in PHASE32_FINAL_SHA256.txt; sizes in MDR) |

## NOT_EXECUTED (honest)

1. Elevated firewall-state probes — requires root (QD-032-001).
2. Live sudoers parse under real permissions on this host — `/etc/sudoers` is
   root-readable only; parser proven via fixtures, live lane reported typed
   NotAvailable (tracked under QD-032-001 umbrella).
3. Engine-live lanes — none exist in this phase.

Everything else executed live on this Mac.
