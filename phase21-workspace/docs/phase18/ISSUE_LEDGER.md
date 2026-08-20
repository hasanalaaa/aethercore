# Phase 18 Issue Ledger

## Closed during implementation

1. Broad Display-class mutation prohibition conflicted with context-aware authority policy. Closed by preserving firmware as protected while moving known-GPU handling into authority/provider recommendation; exact WUA can remain valid for an unknown display vendor.
2. Phase 18 localization insertion temporarily broke catalog structure. Closed before delivery and protected by exact EN/AR key parity audit.
3. New Phase 18 driver rule IDs initially replaced stable Phase 17 IDs and regressed lifecycle/audit history. Closed by retaining `P17-DRV-001/002/003` and versioning semantics to v2.
4. Phase 18 reduced-motion CSS initially introduced `!important`, violating an existing design invariant. Removed; static validation returned 342/342.
5. Phase 17.1 qualification-debt audit assumed an exact total of 12 entries, preventing legitimate later-phase debt extension. Gate made forward-compatible while still requiring all original IDs and uniqueness.
6. WUA network loss was initially indistinguishable from a generic provider failure. Closed by preserving official `WU_E_NO_CONNECTION` (`0x8024001F`) as typed `Offline` authority coverage, so inventory remains available without a false “up to date” result.

## Open qualification debt (not source defects)

Live Windows provider behavior, Authenticode/publisher proof, NTFS staging attack proof, real install/backup/restore/rollback/reboot/post-verification, official GPU utility runtime behavior, future DirectTrusted privileged handoff, and native UI/RTL/accessibility/scale qualification remain in `QUALIFICATION_DEBT.json`.
