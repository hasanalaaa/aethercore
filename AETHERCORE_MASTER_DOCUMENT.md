# AETHERCORE MASTER DOCUMENT

## Executive Summary
AetherCore is a cross-platform system maintenance and intelligence desktop application with a headless automation surface. It provides a universal platform foundation across macOS, Linux, and Windows with honest per-platform capability reporting. At its core, AetherCore utilizes an embedded local intelligence engine (Qwen2.5-1.5B via llama-cpp-2) and enforces a strict, read-only-first diagnostics contract to safely orchestrate performance tuning, hardware diagnostics, driver servicing, and autonomous maintenance without compromising system integrity.

## Architectural Map
The AetherCore architecture is designed around a hardened operation kernel, a cross-platform IPC boundary, specialized telemetry and intelligence domains, and modern frontends. 

### Frontends & Apps
- **Svelte UI (`apps/ui`)**: The primary cross-platform frontend interface built with Svelte 5 and Tauri.
- **aetherctl (`apps/aetherctl`)**: Headless CLI for diagnostics, telemetry querying, capability reporting, journal exporting, and DB checks.
- **Desktop (`apps/desktop`)**: The native desktop app shell.
- **Support Apps**: `consent-broker`, `install-hardener`, `update-broker`.

### Core Services & IPC
- **Maintenance Daemon (`services/maintenance-service`)**: The core operation kernel serving a v7 byte-identical wire contract over named pipes (Windows) and Unix domain sockets (macOS/Linux).
- **IPC (`crates/ipc`)**: Defines the strict boundary and communication contracts between the UI, CLI, and the headless daemon.

### Intelligence Domains
- **Embedded Local Intelligence**: `crates/pc-intelligence`, `crates/timeline-intelligence`, `crates/intelligence-core` (Provides deterministic rule fallbacks and Qwen2.5 integrations).
- **Windows Repair Intelligence**: `crates/windows-repair-intelligence`.
- **Care Orchestrator**: `crates/care-orchestrator`.

### Telemetry & Optimization
- **Hardware & Performance Telemetry**: `crates/hardware-telemetry`, `crates/performance-telemetry`.
- **Bottleneck & Optimization**: `crates/performance-bottleneck`, `crates/performance-optimization`, `crates/idle-scheduler`.

### Security, Diagnostics & System Repair
- **Security & Audit**: `crates/security`, `crates/security-audit`.
- **Diagnostics**: `crates/diagnostics`, `crates/crash-diagnostics`, `crates/diagnostic-engine`, `crates/db-diagnostics` (Read-only SQLite/PG/MySQL linting).
- **Driver Operations**: `crates/driver-authority`, `crates/driver-acquisition`, `crates/driver-hub`, `crates/driver-backup`, `crates/driver-install`.
- **System Maintenance**: `crates/cleaner`, `crates/system-repair`, `crates/startup-manager`, `crates/restore-point`.

## Development State
The project has successfully completed up to **Phase 32**, with early indicators of Phase 33 infrastructure.
- **Phases 0–16**: Core foundation, IPC v7, and desktop app wiring.
- **Phases 17–19**: Deep-scan coordinator, crash diagnostics, hardware telemetry, and Windows repair intelligence.
- **Phases 20–22**: Performance telemetry, bottleneck analysis, optimization governance, idle scheduler, and One-Click Care.
- **Phases 23–25**: Embedded Qwen2.5-1.5B integration, intelligence hardening, and release engineering.
- **Phases 26–30**: Universal capability matrix, native Unix providers, headless CLI (`aetherctl`), signed exports, and read-only DB diagnostics.
- **Phase 31 (The Perfection Pass)**: Documentation integrity, CLI i18n, fuzz testing, and debt consolidation.
- **Phase 32**: Introduction of the Security Audit domain, live validation proofs (GD proofs), strict read-only vulnerability DB testing, and binary-safe deliverables mapping. The architecture strictly enforces read-only contracts before mutation.

## Clean Directory Structure
The extracted core source archive (`AetherCore_Clean_Source.zip`) maps to the following essential structure, stripped of all build artifacts (`target/`, `node_modules/`, `dist/`), OS caches, and temporary dumps:

```text
AetherCore/
├── apps/
│   ├── aetherctl/         # Headless CLI
│   ├── consent-broker/
│   ├── desktop/           # Native Desktop App
│   ├── install-hardener/
│   ├── ui/                # Svelte 5 / Tauri Frontend
│   └── update-broker/
├── crates/                # Core Rust Modules
│   ├── care-orchestrator/
│   ├── diagnostics/       # DB, Crash, Engine Diagnostics
│   ├── driver-*/          # Driver servicing & backup
│   ├── hardware-telemetry/
│   ├── intelligence-*/    # AI & Deterministic intelligence
│   ├── ipc/               # Inter-process communication
│   ├── operation-*/       # Operation kernel and engine
│   ├── performance-*/     # Telemetry & Optimization
│   ├── platform-capabilities/
│   ├── security-audit/
│   └── ... (40+ crates)
├── docs/                  # System Documentation & Phase Reports
│   ├── phase17/ - phase32/
│   ├── adr/
│   └── ARCHITECTURE.md, THREAT_MODEL.md, etc.
├── fuzz/                  # Fuzz testing targets
├── installer/             # Installer scripts/configs
├── packaging/             # Packaging tools
├── release/               # Release pipelines
├── scripts/               # Utility & Verification Scripts
├── services/
│   └── maintenance-service/ # Core Daemon
├── tests/                 # Integration tests
├── tools/                 # Internal tools
├── Cargo.toml             # Workspace configuration
└── README.md              # Project manifest
```
