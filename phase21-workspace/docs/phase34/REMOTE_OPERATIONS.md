# Phase 34 — Remote Operations

## Closed operation set

| Operation | Remote command words | Mutates remote? |
|---|---|---|
| `VersionProbe` | `aetherctl --version` | No |
| `Capabilities` | `aetherctl --output json capabilities` | No |
| `SecurityAudit` | `aetherctl --output json sec audit --targets-json <validated>` | No |
| `ComplianceCollect` | `aetherctl --output json sec audit --profile <cis-l1\|cis-l2> --format json` | No |
| `ReportPrint` | `cat <validated path>` | No |

There is no `fleet exec`. The set is a Rust enum; adding a variant requires a
code change plus audit-gate review. Target JSON is validated (array, ≤64
items, only `kind`/`path` keys, kind allowlist, path charset). Remote paths
must be absolute and free of whitespace/metacharacters — and are
single-quoted regardless.

## Injection resistance

- Local side: `Command` with individual argv elements — no shell.
- Remote side: every word POSIX single-quoted (`shell_quote`), embedded
  quotes escaped; hostname/username validated to exclude `; | & $ \` " ' \`
  spaces, option prefixes, `user@`, schemes.
- Proofs: `shell_quote_neutralizes_metacharacters`,
  `argv_is_safe_and_strict`, audit gates `p34-injection-resistant-command-builder`.

## Transport properties

- OS OpenSSH client (no alternate SSH stack).
- `BatchMode=yes` (no prompts), `StrictHostKeyChecking=yes`,
  `PasswordAuthentication=no`, `KbdInteractiveAuthentication=no`.
- `ConnectTimeout` explicit; per-operation deadline with kill-on-expiry →
  typed `Timeout`.
- `CancelToken` → typed `Cancelled` (checked every ~20 ms).
- stdout/stderr captured with a 256 KiB bound per stream and an honest
  `truncated` marker.
- ssh missing → typed `NotAvailable` path (`TransportError::SshMissing` /
  `RemoteOutcomeKind::NotAvailable`); no silent degradation.

## Compatibility handshake

Before substantive operations, a `VersionProbe` result parsed by
`parse_compatibility` yields `RemoteCompatibility { aetherctl_version,
remote_schema, supported }` against `REMOTE_CONTRACT_VERSION =
fleet.remote.v1`. Unparseable or mismatched remotes are typed
`Incompatible` and the operation stops. Capability is proven from the remote
envelope schema — not inferred from semver alone.

## Result state space

`Success | Failed | NotVerified | NotAvailable | Timeout | Cancelled |
AuthFailure | HostKeyMismatch | Incompatible` — exactly the honest
distinctions the phase requires, mapped deterministically from exit codes
and stderr shapes (`classify_ssh_exit`).

Typed outcome states surfaced by the Fleet desktop page (verbatim, EN/AR):

```json
{
  "outcomeStates": [
    "NotVerified",
    "NotAvailable",
    "HostKeyMismatch",
    "Timeout",
    "AuthFailure",
    "Incompatible"
  ]
}
```

Remote audit/compliance remains READ-ONLY from every surface, including the
desktop management actions introduced in the corrective closure.
