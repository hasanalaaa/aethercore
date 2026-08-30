# Synthetic Windows Repair Lab

`crates/windows-repair-intelligence/tests/synthetic_lab.rs` contains deterministic source tests for P19-01 through P19-24:

- healthy/no fabricated repair;
- component-store repair;
- component-before-SFC ordering;
- verification failure truth;
- reboot barrier;
- servicing-before-update;
- offline update truth;
- targeted service repair;
- unexpected service configuration;
- DNS-specific handling;
- proxy review;
- DHCP/network separation;
- Winsock safety/reboot contract;
- filesystem vs hardware separation;
- recovery protection loss;
- WinRE readiness;
- failures before/after mutation;
- stale state after consent;
- mutation-supervisor contract;
- safe/deferred cancellation outcome vocabulary;
- escalation after lower repair exhaustion.

Additional invariants reject cycles, missing dependencies, destructive SAFE_AUTO, confident diagnosis from unknown evidence, global corruption from one failed collector, HRESULT-only root cause authority, missing recovery protection, automatic post-reboot continuation and contradictory destructive recovery.

The Rust tests are authored but were not executed in this delivery environment because `cargo` and `rustc` are absent. The independent Python Phase 19 semantic/source audit is executed separately and does not substitute for Windows-native qualification.
