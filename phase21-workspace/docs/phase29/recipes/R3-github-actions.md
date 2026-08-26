# R3 — GitHub Actions self-hosted runner job (linux/macos matrix)

> Documentation-grade recipe. GitHub Actions execution is NOT_EXECUTED on this macOS
> host — static lint only (QD-028-002 analogue). Copy into
> `.github/workflows/nightly-maintenance.yml` on the fleet repo.

```yaml
name: nightly-maintenance

on:
  schedule:
    - cron: "15 0 * * *"   # 00:15 UTC nightly; runner-local time applies for self-hosted
  workflow_dispatch:

jobs:
  maintenance:
    strategy:
      matrix:
        os: [self-hosted-linux, self-hosted-macos]
    runs-on: ${{ matrix.os }}
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@v4

      - name: Run R1 nightly recipe
        id: r1
        shell: bash
        env:
          AETHERCTL: ${{ runner.temp }}/aetherctl-bin/aetherctl
        run: |
          bash docs/phase29/recipes/R1-nightly-maintenance.sh

      - name: Upload signed/digested export as artifact
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: aethercore-export-${{ matrix.os }}-${{ github.run_id }}
          path: |
            ~/.local/share/aethercore-nightly/exports/journal-*.json
            ~/.local/share/aethercore-nightly/exports/verify-*.log
          if-no-files-found: warn
```

Expected output per matrix leg: R1 prints `[R1] OK — verified export at …` and exits 0;
the artifact contains one journal JSON plus its verify log.
