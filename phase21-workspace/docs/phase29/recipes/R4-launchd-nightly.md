# R4 — launchd nightly equivalent for macOS fleets

> Documentation-grade artifact. Nothing installs this automatically. Complements
> `packaging/launchd/com.aethercore.maintenance.plist` (the daemon agent).

## ~/Library/LaunchAgents/com.aethercore.nightly-maintenance.plist

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.aethercore.nightly-maintenance</string>

  <key>ProgramArguments</key>
  <array>
    <string>/bin/bash</string>
    <string>/opt/aethercore/share/recipes/R1-nightly-maintenance.sh</string>
  </array>

  <key>EnvironmentVariables</key>
  <dict>
    <key>AETHERCTL</key>
    <string>/opt/aethercore/bin/aetherctl</string>
  </dict>

  <key>StartCalendarInterval</key>
  <dict>
    <key>Hour</key><integer>3</integer>
    <key>Minute</key><integer>15</integer>
  </dict>

  <!-- launchd has no Persistent=true equivalent for calendar jobs; missed runs fire
       at the next matching calendar tick. KeepAlive=false: this is a one-shot job. -->
  <key>RunAtLoad</key>
  <false/>
  <key>KeepAlive</key>
  <false/>

  <key>StandardOutPath</key>
  <string>~/.local/share/aethercore-nightly/launchd.out.log</string>
  <key>StandardErrorPath</key>
  <string>~/.local/share/aethercore-nightly/launchd.err.log</string>
</dict>
</plist>
```

## Install / uninstall

```bash
mkdir -p ~/.local/share/aethercore-nightly
plutil -lint com.aethercore.nightly-maintenance.plist          # static lint (run here)
cp com.aethercore.nightly-maintenance.plist \
   ~/Library/LaunchAgents/                                     # install
launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.aethercore.nightly-maintenance.plist 2>/dev/null || true
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.aethercore.nightly-maintenance.plist
launchctl print gui/$(id -u)/com.aethercore.nightly-maintenance | head   # verify

# uninstall
launchctl bootout gui/$(id -u)/com.aethercore.nightly-maintenance || true
rm ~/Library/LaunchAgents/com.aethercore.nightly-maintenance.plist
```
