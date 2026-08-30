# Phase 17 Remediation Model

`RemediationCandidate` is descriptive and non-executing. Each candidate binds to a finding and carries action type, authority, privilege, safety, reversibility, reboot expectation, expected effect, preconditions, verification method, conflict list, duration category and automatic eligibility.

Safety classes are `SafeAuto`, `SafeReview`, `Sensitive`, `Manual`, and `HardwareService`. Driver replacement and app installation are never silent `SafeAuto`; hardware-service conditions are never represented as software-fixable.

`RemediationPlan::seal` is the Phase 17 immutable-plan foundation. Before sealing, the client may select/deselect candidates. Sealing sorts actions by stable ID, serializes the exact action set with scan ID and creation timestamp, computes SHA-256, derives plan identity from that digest, marks the plan immutable, and persists it with a foreign-key reference to the originating scan.

Phase 17 intentionally has no route that hands this plan to `MutationSupervisor` for execution. Future remediation phases must consume the sealed identity through existing one-shot consent, operation-kernel, CommitFence and journal boundaries rather than mutating the plan in place.
