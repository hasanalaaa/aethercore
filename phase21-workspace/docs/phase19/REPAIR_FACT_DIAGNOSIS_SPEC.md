# Repair Fact & Diagnosis Specification

## Typed fact domains

The model covers component store, protected system files, servicing, Windows Update, reboot, services, network, DNS, proxy, filesystem, storage hardware, recovery, boot, Windows Security, application platform and driver handoff.

`FactState` includes healthy/available, repairable/corruption, source-required, repair-failed, reboot-required, active/stopped/disabled, unexpected configuration, offline/failure/degraded and unknown.

## Diagnosis truth

A `Diagnosis` contains:

- `role`: `RootCause`, `ContributingCondition`, or `Symptom`;
- domain and scope;
- evidence IDs;
- `Unknown/Low/Medium/High/Confirmed` confidence;
- uncertainty text;
- versioned rule ID.

Examples encoded in source:

- component-store corruption can be a root cause;
- system-file corruption becomes a symptom when component-store corruption is already supported;
- a pending reboot is a contributing servicing condition, never corruption;
- WUA offline is an update symptom, not evidence of update-store corruption;
- filesystem failure and storage-hardware degradation remain separate domains;
- an unknown collector creates no confident diagnosis.

Error classification retains technical HRESULT/Win32 evidence without treating an error-code mapping alone as root-cause authority.
