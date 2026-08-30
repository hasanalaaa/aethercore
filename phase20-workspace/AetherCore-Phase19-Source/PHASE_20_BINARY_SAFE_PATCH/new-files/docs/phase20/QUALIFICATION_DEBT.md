{
  "schema": "aethercore.phase20.qualification-debt.v1",
  "closureClaim": "PHASE_20_PERFORMANCE_INTELLIGENCE_SOURCE_COMPLETE",
  "items": [
    {
      "id": "P20-QD-001",
      "area": "Domain A — PDH collection",
      "debt": "WindowsPerfPlatform uses live PDH English counters and CallNtPowerInformation; counter availability varies by Windows edition/locale and vendor drivers. Live validation of every collector on real hardware is pending.",
      "risk": "medium",
      "evidenceClass": "SOURCE_COMPLETE"
    },
    {
      "id": "P20-QD-002",
      "area": "Domain A — GPU/VRAM",
      "debt": "GPU engine utilization falls back to aggregate 3D-engine counters; adapter identity and VRAM budget are populated from hardware-telemetry inventory in the full product path. DXGI adapter traversal depth is deferred.",
      "risk": "low",
      "evidenceClass": "SOURCE_COMPLETE"
    },
    {
      "id": "P20-QD-003",
      "area": "Domain C — EcoQoS / priority application",
      "debt": "The shipped OptimizationPlatform for the service is the audit-only NoopPlatform: the planner/journal/restore path is fully implemented and adversarially tested, but PROCESS_POWER_THROTTLING and SetPriorityClass calls require live Windows qualification before enablement.",
      "risk": "high",
      "evidenceClass": "QUALIFICATION_PENDING",
      "note": "This is deliberate: shipping unverified process-throttling against arbitrary user processes would violate the safety contract."
    },
    {
      "id": "P20-QD-004",
      "area": "Domain C — Game Mode registry semantics",
      "debt": "Game Mode toggle writes the sanctioned policy value only after consent; per-Windows-release policy-value behavior needs native verification.",
      "risk": "medium",
      "evidenceClass": "QUALIFICATION_PENDING"
    },
    {
      "id": "P20-QD-005",
      "area": "Domain D — WebView2 rendering",
      "debt": "svelte-check is clean (0 errors); live 60/120 Hz fluidity, Narrator, and RTL layout on WebView2 remain UI qualification debt inherited from Phase 19.",
      "risk": "low",
      "evidenceClass": "QUALIFICATION_PENDING"
    },
    {
      "id": "P20-QD-006",
      "area": "Test harness scheduling",
      "debt": "Full-workspace `cargo test` with default parallelism can surface a spurious SQLite DatabaseBusy in one driver-install test when many test binaries contend concurrently; the test passes in isolation and with --jobs 2. Product code is unaffected (service opens its database single-process).",
      "risk": "low",
      "evidenceClass": "KNOWN_ISSUE"
    }
  ]
}
