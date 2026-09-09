# AetherCore

Cross-platform system maintenance and intelligence desktop application with a headless
automation surface. Universal platform foundation across **macOS / Linux / Windows**
with honest per-platform capability reporting, an embedded local intelligence engine,
and a hard read-only-first diagnostics contract.

## What AetherCore is today

- **Maintenance daemon** (`aethercore-maintenance-service`): one operation kernel
  serving the SAME wire contract (v7, byte-identical framing) over named pipes on
  Windows and Unix domain sockets on macOS/Linux.
- **aetherctl** — headless CLI: doctor · perf · optimize · timeline · care · insights ·
  scan · capabilities · export journal/verify · keys generate/fingerprint · db check.
  EN + AR output; JSON envelope `aethercore.aetherctl.v1`.
- **Embedded Qwen2.5-1.5B intelligence** (llama-cpp-2, q4_k_m GGUF) for on-device
  insights; deterministic rule fallback when absent.
- **EXPORT_V1** — canonical hash-chained journal exports with OPTIONAL Ed25519 signing
  (`keys generate` is explicit; unsigned exports say so).
- **Database diagnostics** — `db diagnostics` domain (read-only first): SQLite integrity/config-lint/slow-log diagnostics proven live;
  PostgreSQL/MySQL config lints + slow-log analyzers (file-based, fixture-proven).
- **Signed delivery chain**: every phase ships a hash-manifest binary-safe patch, a
  full-tree SHA-256 ledger, dual-mode verifier, and a twice-identical deterministic
  archive.

## Quick start

### macOS / Linux (daemon + CLI)

```bash
# build
cargo build --release -p aethercore-maintenance-service \
  --features aethercore-maintenance-service/unix-ipc
cargo build --release -p aetherctl

# start the daemon (foreground; Ctrl-C drains cleanly)
./target/release/aethercore-maintenance-service --foreground

# in another shell
./target/release/aetherctl service detect
./target/release/aetherctl --output json capabilities
./target/release/aetherctl doctor
```

### Windows

```powershell
cargo build --release -p aethercore-maintenance-service -p aetherctl
./target/release/aethercore-maintenance-service.exe --foreground
./target/release/aetherctl.exe --output json capabilities
```

### Offline diagnostics (no daemon required)

```bash
aetherctl db check --sqlite ./path/to/db.sqlite
aetherctl export verify ./journal-export.json
aetherctl keys generate --out ./export-signing.key
```

## Verification

```bash
cargo test --workspace --jobs 2          # full suite
python3 scripts/phase30-adversarial-audit.py .   # adversarial audit (652 checks)
python3 PHASE_30_BINARY_SAFE_PATCH/verify_phase30.py . --full-tree   # byte-level tree proof
python3 scripts/phase16-ga-audit.py                 # Phase 16 release qualification source gate
python3 scripts/zenith-adversarial-audit.py         # interaction/accessibility source gate
```

**Phase 16 production qualification:** `scripts/verify-production.ps1` is the
authoritative Windows release seal. The Windows installer is produced by `scripts/build-release.ps1` as
`AetherCoreSetup-<version>-x64.exe`. A signed production setup requires the
Windows qualification and signing steps in `docs/FINAL_PRODUCTION_QUALIFICATION.md`;
macOS/Linux builds do not claim to produce a Windows installer.

To build an installable Windows candidate from the current commit, open the
repository on GitHub, choose **Actions → Windows installer candidate → Run
workflow**, then download the `AetherCoreSetup-windows-unsigned-*` artifact.
This is an unsigned test package. On Windows, extract the artifact and run the
`AetherCoreSetup-<version>-x64.exe` file as administrator. Production users
must use a signed bundle from the protected release workflow.

## Platform capability matrix (honest)

| Capability | macOS | Linux | Windows |
|---|---|---|---|
| Telemetry CPU/Memory | native | native | native |
| Telemetry Storage | native (statfs) | Degraded (proxies) | native |
| Telemetry GPU | Degraded | Degraded | native |
| Driver servicing | NotAvailable | NotAvailable | native |
| System repair (DISM/SFC) | NotAvailable | NotAvailable | native |
| Restore points | NotAvailable | NotAvailable | native |
| DB diagnostics | native (SQLite) / file-lint (PG·MySQL) | same | same |
| Care orchestration | native | native | native |

Full typed matrix: `crates/platform-capabilities`; surfaced live via
`aetherctl capabilities` and the desktop About panel.

## Phase history

| Phase | One-line summary |
|---|---|
| P0–P16 | Foundation: kernel, IPC v7, domains, desktop app. |
| P17/17.1 | Deep-scan coordinator + evidence-cited finding discipline. |
| P18/18.1 | Crash diagnostics + hardware telemetry foundations. |
| P19 | Windows repair intelligence source completion. |
| P20 | Performance telemetry ring + bottleneck analysis (adversarial-hardened). |
| P21 | Optimization governance + idle scheduler. |
| P22 | One-Click Care orchestration journal. |
| P23 | Embedded Qwen2.5-1.5B local intelligence (GGUF pinned). |
| P23.1 | Intelligence hardening seal. |
| P24 | Windows qualification lane (manifest: docs/P24_WINDOWS_TEST_MANIFEST.md). |
| P25 | Release engineering groundwork. |
| P26 | Universal platform foundation: capability matrix, Transport trait, UDS scaffold. |
| P27 | Native unix providers (macOS libc / Linux proc) + UDS composition wired. |
| P28 | aetherctl headless CLI + daemon hardening + systemd/launchd units. |
| P29 | EXPORT_V1 signed exports + structured logging registry + automation recipes. |
| P30 | Database diagnostics domain (read-only first). |
| P31 | Phase 31 — The Perfection Pass: documentation integrity restoration (phase29/30 docs authored), DEBT_REGISTER.json consolidation, CLI i18n EN+AR, fuzz into CI, cargo-deny, coverage/bench baselines, correlation ids. **Current phase.** |

Phase docs: `docs/phase<NN>/ARCHITECTURE.md`, `SCORECARD.md`,
`MASTER_DELIVERY_REPORT.md`.

## Enterprise surfaces

- Consolidated debt register: [DEBT_REGISTER.json](DEBT_REGISTER.json)
- Extensibility contract: [EXTENSIBILITY.md](EXTENSIBILITY.md)
- Windows qualification checklist: [docs/P24_WINDOWS_TEST_MANIFEST.md](docs/P24_WINDOWS_TEST_MANIFEST.md)
- Fuzz targets: `fuzz/` (5 parsers) via `.github/workflows/fuzz.yml`

## License & governance

See docs/RELEASE_SUPPLY_CHAIN.md and docs/THREAT_MODEL.md. Supply-chain policy:
`cargo deny` (RustSec=deny); dependency waivers carry owner-review comments in-tree.
