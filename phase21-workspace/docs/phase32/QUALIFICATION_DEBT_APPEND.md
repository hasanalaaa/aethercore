# Phase 32 — QUALIFICATION DEBT (append-only additions)

Append-only record per house policy. The authoritative machine-readable
register is `QUALIFICATION_DEBT.json` at the workspace root (items QD-032-001,
QD-032-002, QD-032-003 appended this phase; nothing closed without evidence;
no pre-existing entries modified).

## QD-032-001 — Live firewall state needs an elevation lane (open)

- **What runs today:** pf/ufw/nftables detection via CONFIG FILES only
  (`fw.config_present`, confidence=inferred, Advisory). Absence of any config →
  honest `NotAvailable` marker finding (`fw.state_not_available`) naming
  QD-032-001 in the reason string.
- **What cannot run on this host:** rule-state enumeration (`pfctl -s all`,
  `ufw status verbose`, `nft list ruleset`) — all require elevation. This Mac
  is the unix proof host and elevation is out of scope for the read-only
  contract.
- **Qualification path:** elevated probe lane (owner-approved), then a
  config-vs-state divergence study; until then config-presence must be read as
  "a firewall is configured here", never "the firewall is enforcing".

## QD-032-002 — macOS password-policy paths differ by construction (open)

- **What runs today:** login.defs-style file lint (`pass.*` rules) proven on
  Linux fixtures; on macOS the lane returns typed
  `NotAvailable("macOS stores password policy outside login.defs
  (OpenDirectory pwpolicy); file-based lint is NotAvailable on this platform")`.
- **Why:** macOS has no `/etc/login.defs`; policy lives in OpenDirectory.
  Pretending otherwise would violate the anti-snake-oil contract.
- **Qualification path:** OpenDirectory-backed read provider + Linux
  pam_pwquality cross-check matrix.

## QD-032-003 — CVE DB freshness governance (open)

- **What runs today:** 36-entry NVD-derived seed pinned by sha256 manifest;
  load is fail-closed (hash + entry count); updates ONLY via explicit owner
  CLI action with validate-before-write and rollback; runtime network banned.
- **What is missing:** owner-approved governance defining refresh cadence,
  review before pin rotation, and rollback pin retention. Without it the DB is
  honest but ages — the report digest makes staleness visible but does not fix
  it.
- **Qualification path:** governance note + two supervised manifest rotations
  through the CLI path.

## NOT_EXECUTED recorded honestly this phase

1. **Elevated firewall-state probes** — requires root; out of scope (above).
2. **Live sudoers parse under real permissions** — GD-4 ran against the REAL
   `/etc/sudoers`, which is root-readable only; the lane reported typed
   NotAvailable(`Permission denied`) rather than fabricating results. The
   parser itself is fully proven via fixtures (GD suite). An elevated
   qualification run remains open under the same umbrella as QD-032-001.
3. **Engine-live lanes** — none exist in this phase; nothing hidden here.

Everything else executed live on this Mac: cargo test ×2 identical greens,
GD-1..GD-5 including the real-host audit digest stability ×2 and the live
tamper test.
