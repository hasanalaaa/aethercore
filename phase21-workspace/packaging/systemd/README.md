# packaging/systemd — honest verification status

`aethercore-maintenance.service` is a Phase 28 LIFECYCLE ARTIFACT (CX-5). It is never
installed automatically.

Verification performed on this host (macOS):

- Static key assertions (Type=simple, ExecStart carries `--daemon`, Restart=on-failure,
  NoNewPrivileges/ProtectSystem/PrivateTmp hardening present) run inside
  `scripts/phase28-adversarial-audit.py`.

Verification NOT executed on this host:

- `systemd-analyze verify` requires a Linux host with systemd; it is recorded as
  NOT_EXECUTED in docs/phase28/QUALIFICATION_DEBT.json under QD-028-002. Do not claim
  runtime validity until that runs on Linux.
