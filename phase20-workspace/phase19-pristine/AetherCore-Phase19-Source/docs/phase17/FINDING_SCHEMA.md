# Phase 17 Finding Schema

A `Finding` is a user-relevant conclusion supported by facts. It contains:

- deterministic finding ID and stable finding code;
- domain;
- severity: Informational, Low, Moderate, High, Critical;
- confidence: Unknown, Low, Medium, High, Confirmed;
- localized title/summary/technical message keys and typed string arguments;
- evidence references and affected resource;
- first/last observed timestamps;
- lifecycle: New, Active, Improved, Resolved, Returned, Ignored;
- remediation availability and safety;
- privilege, reboot, automatic eligibility, reversibility and impact metadata;
- uncertainty message key;
- rule ID/version.

Severity never substitutes for confidence. Diagnostic unavailability is represented as a limitation fact/collector status and is not transformed into an unhealthy-machine finding.

The overall system status is categorical only: Healthy, Attention Recommended, Action Required, Critical. No generic PC-health percentage exists in Phase 17.
