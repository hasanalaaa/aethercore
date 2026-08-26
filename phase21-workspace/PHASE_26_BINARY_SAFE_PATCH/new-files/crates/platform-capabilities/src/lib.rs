//! Phase 26 — Universal Platform Foundation: typed capability matrix.
//!
//! CX contract: Windows behavior is FROZEN (everything Native there, byte-for-byte
//! unchanged); other platforms report their CURRENT reality honestly. `NotAvailable`
//! is a first-class typed answer — this crate NEVER simulates an unavailable capability.

use serde::{Deserialize, Serialize};

/// Every capability the product can expose, platform-independent.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PlatformCapability {
    TelemetryCpu,
    TelemetryMemory,
    TelemetryStorage,
    TelemetryGpu,
    ThermalPowerClamp,
    DriverServicing,
    SystemRepairDism,
    SystemRepairSfc,
    SystemRepairWua,
    ProcessGovernorEcoQos,
    GameModeProfile,
    RestorePoints,
    WindowsUpdate,
    TimelineIntelligence,
    LocalIntelligence,
    CareOrchestration,
}

impl PlatformCapability {
    /// Stable wire/name string for logs and the capability proto.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TelemetryCpu => "telemetryCpu",
            Self::TelemetryMemory => "telemetryMemory",
            Self::TelemetryStorage => "telemetryStorage",
            Self::TelemetryGpu => "telemetryGpu",
            Self::ThermalPowerClamp => "thermalPowerClamp",
            Self::DriverServicing => "driverServicing",
            Self::SystemRepairDism => "systemRepairDism",
            Self::SystemRepairSfc => "systemRepairSfc",
            Self::SystemRepairWua => "systemRepairWua",
            Self::ProcessGovernorEcoQos => "processGovernorEcoQos",
            Self::GameModeProfile => "gameModeProfile",
            Self::RestorePoints => "restorePoints",
            Self::WindowsUpdate => "windowsUpdate",
            Self::TimelineIntelligence => "timelineIntelligence",
            Self::LocalIntelligence => "localIntelligence",
            Self::CareOrchestration => "careOrchestration",
        }
    }

    /// Enumerate all capabilities (deterministic declaration order).
    pub const ALL: &'static [PlatformCapability] = &[
        Self::TelemetryCpu,
        Self::TelemetryMemory,
        Self::TelemetryStorage,
        Self::TelemetryGpu,
        Self::ThermalPowerClamp,
        Self::DriverServicing,
        Self::SystemRepairDism,
        Self::SystemRepairSfc,
        Self::SystemRepairWua,
        Self::ProcessGovernorEcoQos,
        Self::GameModeProfile,
        Self::RestorePoints,
        Self::WindowsUpdate,
        Self::TimelineIntelligence,
        Self::LocalIntelligence,
        Self::CareOrchestration,
    ];
}

/// Honest availability of one capability on one platform.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    /// Fully functional via the platform's own APIs.
    Native,
    /// Functional with reduced fidelity; note_key resolves in renderer catalogs.
    Degraded { note_key: &'static str },
    /// Not implemented on this platform; reason_key is honest, never a simulation.
    NotAvailable { reason_key: &'static str },
}

/// Supported target platforms for the matrix.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}

impl Platform {
    /// The compiling host's platform.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::Macos
        } else {
            Platform::Linux
        }
    }
}

/// Reason/note keys — a closed vocabulary so catalogs stay exhaustive.
pub mod keys {
    pub const WINDOWS_ONLY_API: &str = "cap.reason.windowsOnlyApi";
    pub const WINDOWS_SERVICE_CTX: &str = "cap.reason.windowsServiceContext";
    pub const MACOS_NO_ECOQOS: &str = "cap.reason.macosNoEcoQos";
    pub const MACOS_NO_DISM_SFC_WUA: &str = "cap.reason.macosNoDismSfcWua";
    pub const MACOS_NO_DRIVER_STORE: &str = "cap.reason.macosNoDriverStore";
    pub const MACOS_NO_WINUPDATE: &str = "cap.reason.macosNoWinUpdate";
    pub const MACOS_RESTORE_NA: &str = "cap.reason.macosRestoreNotAvailable";
    pub const MACOS_GPU_LIMITED: &str = "cap.note.macosGpuLimited";
    pub const MACOS_THERMAL_NQ: &str = "cap.note.macosThermalViaNq";
    pub const LINUX_DISTRO_VARIANCE: &str = "cap.note.linuxDistroVariance";
    pub const LINUX_NO_ECOQOS: &str = "cap.reason.linuxNoEcoQos";
    pub const LINUX_NO_DRIVER_STORE: &str = "cap.reason.linuxNoDriverStore";
    pub const LINUX_NO_DISM_SFC_WUA: &str = "cap.reason.linuxNoDismSfcWua";
    pub const LINUX_RESTORE_NA: &str = "cap.reason.linuxRestoreNotAvailable";
}

/// FROZEN Windows table: everything Native, exactly as shipped today.
fn windows_table() -> Vec<(PlatformCapability, Availability)> {
    use PlatformCapability as C;
    C::ALL.iter().map(|c| (*c, Availability::Native)).collect()
}

/// macOS table — current reality after Phase 26 recon.
fn macos_table() -> Vec<(PlatformCapability, Availability)> {
    use PlatformCapability as C;
    use keys as k;
    let n = || Availability::Native;
    let d = |note| Availability::Degraded { note_key: note };
    let na = |reason| Availability::NotAvailable { reason_key: reason };
    vec![
        (C::TelemetryCpu, n()),
        (C::TelemetryMemory, n()),
        (C::TelemetryStorage, n()),
        (C::TelemetryGpu, d(k::MACOS_GPU_LIMITED)),
        (C::ThermalPowerClamp, d(k::MACOS_THERMAL_NQ)),
        (C::DriverServicing, na(k::MACOS_NO_DRIVER_STORE)),
        (C::SystemRepairDism, na(k::MACOS_NO_DISM_SFC_WUA)),
        (C::SystemRepairSfc, na(k::MACOS_NO_DISM_SFC_WUA)),
        (C::SystemRepairWua, na(k::MACOS_NO_DISM_SFC_WUA)),
        (C::ProcessGovernorEcoQos, na(k::MACOS_NO_ECOQOS)),
        (C::GameModeProfile, na(k::WINDOWS_ONLY_API)),
        (C::RestorePoints, na(k::MACOS_RESTORE_NA)),
        (C::WindowsUpdate, na(k::MACOS_NO_WINUPDATE)),
        (C::TimelineIntelligence, n()),
        (C::LocalIntelligence, n()),
        (C::CareOrchestration, n()),
    ]
}

/// Linux table — honest variance note where distro differences apply.
fn linux_table() -> Vec<(PlatformCapability, Availability)> {
    use PlatformCapability as C;
    use keys as k;
    let n = || Availability::Native;
    let d = |note| Availability::Degraded { note_key: note };
    let na = |reason| Availability::NotAvailable { reason_key: reason };
    vec![
        (C::TelemetryCpu, n()),
        (C::TelemetryMemory, n()),
        (C::TelemetryStorage, d(k::LINUX_DISTRO_VARIANCE)),
        (C::TelemetryGpu, d(k::LINUX_DISTRO_VARIANCE)),
        (C::ThermalPowerClamp, d(k::LINUX_DISTRO_VARIANCE)),
        (C::DriverServicing, na(k::LINUX_NO_DRIVER_STORE)),
        (C::SystemRepairDism, na(k::LINUX_NO_DISM_SFC_WUA)),
        (C::SystemRepairSfc, na(k::LINUX_NO_DISM_SFC_WUA)),
        (C::SystemRepairWua, na(k::LINUX_NO_DISM_SFC_WUA)),
        (C::ProcessGovernorEcoQos, na(k::LINUX_NO_ECOQOS)),
        (C::GameModeProfile, na(k::WINDOWS_ONLY_API)),
        (C::RestorePoints, na(k::LINUX_RESTORE_NA)),
        (C::WindowsUpdate, na(k::WINDOWS_ONLY_API)),
        (C::TimelineIntelligence, n()),
        (C::LocalIntelligence, n()),
        (C::CareOrchestration, n()),
    ]
}

/// The public query: `available_on(platform)` per capability.
///
/// macOS storage telemetry is Native (it was mis-keyed in an intermediate draft and is
/// pinned Native by test): DiskArbitration I/O stats are real on macOS.
pub fn available_on(platform: Platform, capability: PlatformCapability) -> Availability {
    let table = match platform {
        Platform::Windows => windows_table(),
        Platform::Macos => macos_table(),
        Platform::Linux => linux_table(),
    };
    let _ = platform; // per-platform overrides live in the tables themselves
    table
        .into_iter()
        .find(|(c, _)| *c == capability)
        .map(|(_, a)| a)
        .unwrap_or_else(|| Availability::NotAvailable {
            reason_key: keys::WINDOWS_ONLY_API,
        })
}

/// Full matrix for the running OS (startup log + wire surface).
pub fn matrix_for_current_platform() -> Vec<(&'static str, Availability)> {
    let platform = Platform::current();
    PlatformCapability::ALL
        .iter()
        .map(|c| (c.as_str(), available_on(platform, *c)))
        .collect()
}
