# R2 — systemd timer pair (complements packaging/systemd/aethercore-maintenance.service)

> Documentation-grade artifact. Nothing installs these units automatically — review
> before install. Runtime `systemd-analyze verify` is NOT_EXECUTED on this macOS host
> (QD-028-002).

## aethercore-nightly.service

```ini
[Unit]
Description=AetherCore nightly maintenance run (aetherctl R1)

[Service]
Type=oneshot
# Review before install: replace User and paths with your deployment's values.
User=aethercore
ExecStart=/opt/aethercore/bin/aetherctl service detect
ExecStart=/opt/aethercore/bin/bash /opt/aethercore/share/recipes/R1-nightly-maintenance.sh
Environment=AETHERCTL=/opt/aethercore/bin/aetherctl
```

## aethercore-nightly.timer

```ini
[Unit]
Description=Nightly schedule for AetherCore maintenance

[Timer]
# 03:15 local time every night.
OnCalendar=*-*-* 03:15:00
Persistent=true
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
```

## Install / uninstall

```bash
sudo cp aethercore-nightly.service aethercore-nightly.timer \
        /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now aethercore-nightly.timer     # install
systemctl list-timers aethercore-nightly.timer           # verify
sudo systemctl disable --now aethercore-nightly.timer    # uninstall
sudo rm /etc/systemd/system/aethercore-nightly.{service,timer}
```
