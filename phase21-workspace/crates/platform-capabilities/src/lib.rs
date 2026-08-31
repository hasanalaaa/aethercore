//! Phase 26 — Universal Platform Foundation: typed capability matrix.
//!
//! CX contract: Windows workstation behavior stays Native; Server SKUs report their
//! CURRENT reality honestly. `NotAvailable`
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

/// Windows product family used to select the SKU-aware capability table.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WindowsSku {
    Workstation,
    Server,
    ServerCore,
    Unknown,
}

/// Classifies the documented ProductType/InstallationType registry values.
pub fn classify_windows_sku(product_type: u32, installation_type: &str) -> WindowsSku {
    match product_type {
        1 => WindowsSku::Workstation,
        2 | 3 if installation_type.eq_ignore_ascii_case("server core") => WindowsSku::ServerCore,
        2 | 3 => WindowsSku::Server,
        _ => WindowsSku::Unknown,
    }
}

/// Maps the documented `ProductOptions\ProductType` REG_SZ to the numeric
/// `wProductType` (`VER_NT_*`) that `classify_windows_sku` expects.
///
/// WHY THIS EXISTS. `current_windows_sku()` originally read a `ProductType`
/// **DWORD** from `SOFTWARE\Microsoft\Windows NT\CurrentVersion`. No such value
/// exists on Windows -- measured on Windows 11 Pro build 26200, where that name
/// is ABSENT. The read therefore always failed and the function returned
/// `WindowsSku::Unknown` on EVERY Windows host, workstation and server alike, so
/// the SKU-aware capability table introduced with Windows Server admission never
/// once selected the workstation table on a workstation.
///
/// The authoritative registry source for the numeric product type is
/// `SYSTEM\CurrentControlSet\Control\ProductOptions\ProductType`, a REG_SZ whose
/// documented values map onto `VER_NT_WORKSTATION` (1),
/// `VER_NT_DOMAIN_CONTROLLER` (2) and `VER_NT_SERVER` (3) -- the same numbering
/// Windows Installer exposes as `MsiNTProductType`, which is what
/// `installer/wix/Product.wxs` already gates on. Anything unrecognised maps to 0,
/// which `classify_windows_sku` turns into `Unknown`, preserving fail-closed
/// behaviour.
pub fn product_type_code(product_type_sz: &str) -> u32 {
    if product_type_sz.eq_ignore_ascii_case("WinNT") {
        1
    } else if product_type_sz.eq_ignore_ascii_case("LanmanNT") {
        2
    } else if product_type_sz.eq_ignore_ascii_case("ServerNT") {
        3
    } else {
        0
    }
}

/// Reads the live Windows SKU without falling back to a workstation claim.
pub fn current_windows_sku() -> WindowsSku {
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        use windows::{
            core::PCWSTR,
            Win32::System::Registry::{
                RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
            },
        };

        fn wide(value: &str) -> Vec<u16> {
            value.encode_utf16().chain(std::iter::once(0)).collect()
        }
        fn read_sz(subkey: &str, value: &str) -> Option<String> {
            let key = wide(subkey);
            let name = wide(value);
            let mut bytes = 0u32;
            let status = unsafe {
                RegGetValueW(
                    HKEY_LOCAL_MACHINE,
                    PCWSTR(key.as_ptr()),
                    PCWSTR(name.as_ptr()),
                    RRF_RT_REG_SZ,
                    None,
                    None,
                    Some(&mut bytes),
                )
            };
            if status.is_err() || bytes < 2 || bytes > 4096 {
                return None;
            }
            let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
            let status = unsafe {
                RegGetValueW(
                    HKEY_LOCAL_MACHINE,
                    PCWSTR(key.as_ptr()),
                    PCWSTR(name.as_ptr()),
                    RRF_RT_REG_SZ,
                    None,
                    Some(buffer.as_mut_ptr().cast::<c_void>()),
                    Some(&mut bytes),
                )
            };
            if status.is_err() {
                return None;
            }
            let end = buffer.iter().position(|value| *value == 0).unwrap_or(buffer.len());
            Some(String::from_utf16_lossy(&buffer[..end]))
        }

        // The numeric wProductType lives in ProductOptions as a REG_SZ, not as a
        // DWORD under CurrentVersion. See product_type_code() for why.
        let Some(product_type_sz) =
            read_sz(r"SYSTEM\CurrentControlSet\Control\ProductOptions", "ProductType")
        else {
            return WindowsSku::Unknown;
        };
        let product_type = product_type_code(&product_type_sz);
        let installation_type =
            read_sz(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "InstallationType")
                .unwrap_or_default();
        classify_windows_sku(product_type, &installation_type)
    }
    #[cfg(not(windows))]
    {
        WindowsSku::Unknown
    }
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
    pub const WINDOWS_SERVER_NO_THERMAL_POWER: &str = "cap.reason.windowsServerNoThermalPower";
    pub const WINDOWS_SERVER_NO_GAME_MODE: &str = "cap.reason.windowsServerNoGameMode";
    pub const WINDOWS_SERVER_NO_RESTORE_POINTS: &str = "cap.reason.windowsServerNoRestorePoints";
    pub const WINDOWS_SERVER_WUA_POLICY: &str = "cap.note.windowsServerWsusPolicy";
    pub const WINDOWS_SERVER_CORE_NO_CONSOLE: &str = "cap.note.windowsServerCoreNoConsole";
}

/// FROZEN Windows table: everything Native, exactly as shipped today.
fn windows_table() -> Vec<(PlatformCapability, Availability)> {
    use PlatformCapability as C;
    C::ALL.iter().map(|c| (*c, Availability::Native)).collect()
}

fn windows_server_table(core: bool) -> Vec<(PlatformCapability, Availability)> {
    use PlatformCapability as C;
    use keys as k;
    let n = || Availability::Native;
    let d = |note| Availability::Degraded { note_key: note };
    let na = |reason| Availability::NotAvailable { reason_key: reason };
    vec![
        (C::TelemetryCpu, n()),
        (C::TelemetryMemory, n()),
        (C::TelemetryStorage, n()),
        (C::TelemetryGpu, n()),
        (C::ThermalPowerClamp, na(k::WINDOWS_SERVER_NO_THERMAL_POWER)),
        (C::DriverServicing, n()),
        (C::SystemRepairDism, n()),
        (C::SystemRepairSfc, n()),
        (C::SystemRepairWua, d(k::WINDOWS_SERVER_WUA_POLICY)),
        (C::ProcessGovernorEcoQos, n()),
        (C::GameModeProfile, na(k::WINDOWS_SERVER_NO_GAME_MODE)),
        (C::RestorePoints, na(k::WINDOWS_SERVER_NO_RESTORE_POINTS)),
        (C::WindowsUpdate, d(k::WINDOWS_SERVER_WUA_POLICY)),
        (C::TimelineIntelligence, n()),
        (C::LocalIntelligence, n()),
        (
            C::CareOrchestration,
            if core { d(k::WINDOWS_SERVER_CORE_NO_CONSOLE) } else { n() },
        ),
    ]
}

/// Returns the SKU-aware Windows answer while keeping `available_on(Windows, ..)` frozen for
/// callers that explicitly request the workstation baseline.
pub fn available_on_windows_sku(sku: WindowsSku, capability: PlatformCapability) -> Availability {
    let table = match sku {
        WindowsSku::Workstation => windows_table(),
        WindowsSku::Server => windows_server_table(false),
        WindowsSku::ServerCore => windows_server_table(true),
        // Unknown Windows must not be overstated as a workstation.
        WindowsSku::Unknown => windows_server_table(false),
    };
    table
        .into_iter()
        .find(|(candidate, _)| *candidate == capability)
        .map(|(_, availability)| availability)
        .unwrap_or(Availability::NotAvailable { reason_key: keys::WINDOWS_ONLY_API })
}

/// macOS table — Phase 27 reality: telemetryCpu/Memory are Native via the libc-backed
/// `MacosPerfPlatform` (mach host_statistics64 + sysctl, proven on the unix test host);
/// storage is Native via statfs capacity evidence. GPU/thermal stay honestly Degraded.
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

/// Linux table — Phase 27: telemetryCpu/Memory are Native via `/proc` parsers
/// (`LinuxPerfPlatform`, proven by fixture tests + live /proc reads); storage keeps the
/// honest distro-variance note (QD-026-002 unchanged) because diskstats layouts vary.
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
        .map(|c| {
            let availability = match platform {
                Platform::Windows => available_on_windows_sku(current_windows_sku(), *c),
                _ => available_on(platform, *c),
            };
            (c.as_str(), availability)
        })
        .collect()
}

/// Stable wire label for the current OS and Windows SKU.
pub fn current_platform_name() -> &'static str {
    match Platform::current() {
        Platform::Windows => match current_windows_sku() {
            WindowsSku::Workstation => "windows",
            WindowsSku::Server => "windowsServer",
            WindowsSku::ServerCore => "windowsServerCore",
            WindowsSku::Unknown => "windowsUnknownSku",
        },
        Platform::Macos => "macos",
        Platform::Linux => "linux",
    }
}
