# Phase 32 — Architecture: Security Audit Domain (Read-Only, CIS-Mapped)

## Scope and trust model

Phase 32 adds `crates/security-audit` (`aethercore-security-audit`): an
offline, read-only security audit domain with CIS benchmark mapping. The
domain answers ONE question honestly: *what do this host's configuration
files say about its security posture?*

### What config-lint proves

- A cited line in `/etc/ssh/sshd_config` (verbatim, with line number) proves
  what the file SAYS. If `sshd` actually runs with `-oOverride`, a different
  binary, or a drop-in the operator never told us about, the lint does not
  see it. Findings are claims about FILES.
- Absence of a directive is resolved against documented OpenSSH defaults for
  the version family on this host and marked `confidence=inferred`. It is a
  probability statement, never a proof.

### What config-lint CANNOT prove (and therefore never claims)

- That a daemon reloaded its configuration.
- Live firewall state (`pfctl -s`), connection tracking, or rule evaluation —
  requires elevation; out of scope (QD-032-001).
- macOS password policy — stored in OpenDirectory, not in files this domain
  reads; the lane reports typed NotAvailable on macOS (QD-032-002).
- That a CVE range match means an exploitable binary — package census is
  receipt-level, not binary-level (no build-provenance verification).

Every one of these limits is encoded as first-class `NotAvailable` /
`Advisory` states rather than simulated results (anti-snake-oil rule).

## Rule catalog (implemented rules → CIS refs)

| Rule code | Provider | Default severity | CIS ref |
|---|---|---|---|
| ssh.permit_root_login | sshd | High | CIS L1 5.2.8 |
| ssh.password_authentication | sshd | Medium | CIS L1 5.2.9 |
| ssh.pubkey_authentication | sshd | Medium | unmapped |
| ssh.x11_forwarding | sshd | Low | unmapped |
| ssh.max_auth_tries | sshd | Low/Advisory(absent) | CIS L1 5.2.10 |
| ssh.client_alive | sshd | Advisory | unmapped |
| pass.max_days | password | Medium | CIS L1 5.5.1 |
| pass.min_days | password | Low | CIS L1 5.5.2 |
| pass.min_len | password | Medium | unmapped |
| sudo.nopasswd | sudoers | Advisory | CIS L1 1.3.4 |
| sudo.wildcard_all | sudoers | Low | unmapped |
| fs.world_writable | filesystem | Medium | CIS L1 6.1.1 |
| fs.suid_inventory | filesystem | Advisory | unmapped |
| fs.ssh_dir_perms | filesystem | High | CIS L1 6.1.4 |
| fs.ssh_key_perms | filesystem | Critical | CIS L1 6.1.3 |
| fs.scan_truncated | filesystem | Advisory | unmapped (honest truncation marker) |
| auth.failure_burst | authlog | Advisory (heuristic) | unmapped |
| fw.config_present / fw.state_not_available | firewall | Advisory | unmapped |
| secrets.aws_key / private_key_block / generic_assignment / dotenv / scan_truncated | secrets | Critical/High/Advisory | unmapped |
| cve.vulnerable_package | cve join | High | unmapped |

`assets/vulndb/cis_map.json` is the single source of truth for mappings; the
audit gate enforces `^CIS L[12] \d+\.\d+(\.\d+)?$` or `unmapped` WITH note,
for every implemented rule code.

## Evidence and privacy contract

- Every finding carries ≥1 `EvidenceRef {fact, observed, expected_or_threshold,
  source_location}`; construction refuses zero-evidence findings at the type
  boundary (`SecFinding::try_new`).
- **Secrets privacy contract**: matched secret material enters evidence ONLY
  through `redact()` — first 4 characters kept, remainder masked (`AKIA****…`).
  Private-key detectors capture the BEGIN header marker only; key bodies are
  never read into memory structures that could reach evidence. Enforced by GD-2
  (planted fake AWS key must NOT appear raw anywhere in output) and by audit
  gate p32-no-raw-secret-in-evidence.
- Auth-log findings cite verbatim lines (log lines are not secrets); burst
  analysis is time-windowed and deterministic.

## Determinism

Findings sort by (severity DESC, code ASC, id ASC, first-evidence location
ASC) then clamp to MAX_FINDINGS. The report digest is SHA-256 over each
finding's length-prefixed canonical JSON in that order — stable across runs
on identical inputs (proven live ×2 in GD-4, digest `98de3d20…487576`).

## Hard bounds (same clamp family as P30/P31)

MAX_PARSE_BYTES = 1 MiB per file · MAX_FINDINGS = 512 ·
MAX_FILES_PER_SCAN = 4,096 · MAX_SCAN_MILLIS = 2,000 ·
MAX_EVIDENCE_REFS = 32. Hitting a scan bound produces an explicit
`scan_truncated` finding — coverage is never silently partial.

## vulndb air-gap mechanics

- `assets/vulndb/vulndb.json` — seeded from public NVD-derived advisory data
  during THIS build (same precedent as the P23.1 model download). 36 entries:
  openssh/openssl/zlib/curl/sudo/polkit/systemd/xz/bash/libssh2/paramiko/python.
- `vulndb.manifest.json` pins `{schema, entries, sha256}` of the DB bytes.
  Loading verifies BOTH hash and entry count; ANY mismatch → typed
  `VulnDbError::HashMismatch/EntryCountDrift` → the whole CVE lane degrades to
  `NotAvailable(vulndbIntegrity)` (fail-closed). Proven live in GD-5: one
  flipped byte ⇒ lane down, other lanes unaffected, restore verified.
- Runtime network access is BANNED at three layers: no network symbols in the
  crate (audit gate), allowlist-only dependencies (serde/serde_json/sha2/
  thiserror/regex + dev tempfile), and CLI `vulndb update --url` rejected with
  a typed usage error ("network fetch stays banned").
- Updates are EXPLICIT owner actions: `aetherctl vulndb update --from <file>
  --dest <dir>` validates the candidate BEFORE any write, writes db+manifest
  into the owner-named dir (never implicit paths), rolls back on pin failure.
  The service crate NEVER writes; the audit crate itself is write-free
  (enforced by gate p32-read-only-contract).
- Freshness governance is an open qualification item (QD-032-003): the DB is a
  point-in-time seed, not a feed.

## Wire surface (additive only)

Request tag 90 `run_security_audit` · Response tag 52 `security_audit_response`
· EventKind 29 · EventEnvelope tag 38. Prior tags frozen at their historical
values (anchors asserted by audit gates). Targets travel as typed JSON
(`AuditTarget` enum, `kind`-tagged); traversal (`..`) components are rejected
typed before any provider runs, both in the router and offline lanes.

## Insights integration

`EvidenceSurface::SecurityFinding` extends the local reasoner's evidence pack
(digest tag 4, wire label `securityFinding`). Rule 4 emits a posture summary
insight citing finding ids verbatim — reuse of the existing citation/
resolution machinery, zero forks.
