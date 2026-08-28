# Phase 34 — Security Model

## Threat model

Phase 34 introduces remote reach: the controlling AetherCore host now opens
SSH connections to enrolled fleet hosts. New attack surface is bounded by
five invariants.

### 1. No secrets at rest

The fleet domain schema has no field capable of carrying a password, private
key body, passphrase, or API key. Authentication is modeled as a *reference*:
`agent` (ambient agent resolution), or a local identity-file **path** for
`key_file` / `certificate`. SQLite migration `0015_phase34_fleet` mirrors
this: `auth_ref_kind` + `auth_ref_path` + fingerprint columns only. Audit
gates `p34-no-secret-persistence`, `p34-no-password-auth-storage`,
`p34-no-plaintext-secret-columns` enforce the absence structurally.

### 2. Fail-closed host trust

A host without an explicit pin is `NotVerified` and is never contacted. A
host whose presented key fingerprint differs from its pin is
`HostKeyMismatch` and is hard-blocked. There is no TOFU anywhere. Pins are
created only by the explicit `fleet trust` command with a user-supplied
SHA-256 fingerprint. Trust material lives in an AetherCore-owned
`known_hosts` under the product state directory — the user's
`~/.ssh/known_hosts` is never read or written.

### 3. No arbitrary remote execution

The remote command set is a closed enum (`RemoteOperation`): version probe,
capability listing, security audit with strictly validated JSON targets, and
compliance collection for `cis-l1`/`cis-l2`. There is no `fleet exec`.
Every argv element is constructed by `build_remote_argv` and remote words are
POSIX single-quoted via `shell_quote`; hostnames and usernames are validated
against metacharacter/option-prefix injection at the domain layer.

### 4. Non-interactive, verified transport

ssh runs via typed argv (no local shell) with `BatchMode=yes`,
`StrictHostKeyChecking=yes`, `PasswordAuthentication=no`,
`KbdInteractiveAuthentication=no` and an explicit `ConnectTimeout`. Insecure
equivalents (`StrictHostKeyChecking=no`, `accept-new`,
`UserKnownHostsFile=/dev/null`, `CheckHostIP=no`, password-preference
options) are enumerated in `FORBIDDEN_SSH_OPTION_FRAGMENTS` and asserted
absent by tests and audit gates. Prompts are impossible; password paths are
structurally unreachable.

### 5. Honesty of remote compliance

Reports collected remotely are verified locally with the established P33
pipeline: strict parse (`deny_unknown_fields`), canonical digest check, and
Phase29 Ed25519 signature verification when `signed=true`. Content tamper →
`DigestMismatch`; signature tamper → signature mismatch rejection; unsigned
reports are reported unsigned, never "authenticated". Per-host scores are
never averaged into a false fleet score; any displayed aggregate must define
its denominator explicitly (pass+fail over pass+fail+na+not_verified, per
the P33 score semantics, computed per host and labeled).

## Read-only guarantee

All remote operations are read-only equivalents of existing offline lanes.
No fleet path mutates a remote host. Local mutations are limited to
inventory/schedule administration, which are explicit CLI actions.

## Logging

Error details carry ssh stderr first lines and typed state names only. No
identity file contents, no key material, no credentials are ever logged.
