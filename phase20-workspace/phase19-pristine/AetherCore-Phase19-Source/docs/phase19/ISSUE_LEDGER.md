# Phase 19 Issue Ledger

| ID | Severity | Finding | Resolution / disposition |
|---|---|---|---|
| P19-I-001 | High | Delivered Phase 18.1 narrative SHA could be stale relative to bytes. | Closed: recompute all hashes from final bytes; micro-preflight records actual source ZIP SHA. |
| P19-I-002 | Critical | Old repair truth could treat any failed probe as Windows corruption. | Closed: collector failure normalizes to Unknown; PC Intelligence only creates actionable Windows finding from explicit result codes. |
| P19-I-003 | Critical | Localized DISM/SFC console prose could become health authority. | Closed for DISM with `DismCheckImageHealth`; SFC console prose is not parsed, bounded CBS evidence is isolated and insufficient evidence remains Unknown. Live SFC/CBS behavior retained as qualification debt. |
| P19-I-004 | Critical | Repair plan could become stale after consent. | Closed: assessment ID + canonical fingerprint + RepairGraph digest revalidated before consent consumption. |
| P19-I-005 | Critical | Reboot boundary could allow pre-reboot assumptions to continue. | Closed: graph rejects automatic post-barrier continuation; reboot ticket requires fresh reassessment and is never mutation authority. |
| P19-I-006 | Critical | Renderer could become command/service mutation authority. | Closed: renderer sees trusted action IDs only; executable/args/service target are server-owned. |
| P19-I-007 | High | “Service started” could falsely resolve Windows Update. | Closed: service repair verification also requires a fresh WUA discovery. |
| P19-I-008 | Critical | Destructive recovery might drift into SAFE_AUTO. | Closed: graph invariant rejects destructive SAFE_AUTO and contradictory destructive recovery. |
| P19-I-009 | High | UI could advertise graph nodes that runtime cannot execute. | Closed: review filters to the backend-supported runtime authority set and suppresses execution across reboot barriers. |
| P19-I-010 | Medium | Baseline static validator expected old `/ScanHealth` console workflow. | Closed: validator upgraded to accept the structured DISM API preflight while preserving mutation-barrier ordering. |
| P19-I-011 | High | Phase 17/17.1 regression auditors rewrote tracked evidence files even when used only for verification. | Closed: both auditors are now read-only by default and write evidence only when `--output` is explicitly supplied. |
| P19-D-001 | Qualification debt | Rust toolchain unavailable in delivery environment. | Open qualification debt; no Rust compile/test PASS claimed. |
| P19-D-002 | Qualification debt | Live Windows DISM/SFC/WUA/recovery/reboot/WebView2 behaviors not run here. | Open qualification debt by program strategy; no native PASS claimed. |
| P19-C-001 | Product capability debt | Broader official OEM/component driver automation remains incomplete. | Preserved in `PRODUCT_CAPABILITY_DEBT.json`; not solved in Phase 19. |
| P19-C-002 | Product capability debt | Additional structured network/proxy/DHCP repair provider depth. | Deferred rather than replaced with broad reset/tweak actions. |
