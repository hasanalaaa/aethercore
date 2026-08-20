use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AutonomousWorkload {
    HardwareTelemetry,
    DriverDiscovery,
    CleanupInventory,
    StartupInventory,
    EventLogTriage,
}

impl AutonomousWorkload {
    pub const ALL: [Self; 5] = [
        Self::HardwareTelemetry,
        Self::DriverDiscovery,
        Self::CleanupInventory,
        Self::StartupInventory,
        Self::EventLogTriage,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HardwareTelemetry => "HardwareTelemetry",
            Self::DriverDiscovery => "DriverDiscovery",
            Self::CleanupInventory => "CleanupInventory",
            Self::StartupInventory => "StartupInventory",
            Self::EventLogTriage => "EventLogTriage",
        }
    }

    pub const fn network_sensitive(self) -> bool {
        matches!(self, Self::DriverDiscovery)
    }

    pub const fn thermal_sensitive(self) -> bool {
        matches!(self, Self::DriverDiscovery | Self::CleanupInventory)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThermalPressure { Normal, Elevated, Critical, Unknown }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkCost { Unmetered, Metered, Unknown }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationState { Clear, Busy, FullScreen, Presentation, LockedOrAbsent, Unknown }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServicingState { Idle, Busy, Unknown }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemState {
    pub sampled_unix_ms: i64,
    pub owner_principal_key: String,
    pub user_sid: String,
    pub session_id: u32,
    pub idle_for: Duration,
    pub session_unlocked: bool,
    pub on_ac_power: bool,
    pub battery_saver: bool,
    pub thermal_pressure: ThermalPressure,
    pub network_cost: NetworkCost,
    pub presentation: PresentationState,
    pub servicing: ServicingState,
}

impl SystemState {
    pub fn has_interactive_owner(&self) -> bool {
        !self.owner_principal_key.is_empty() && !self.user_sid.is_empty() && self.session_id != u32::MAX
    }
}

#[derive(Clone, Debug)]
pub struct SchedulerConfig {
    pub minimum_idle: Duration,
    pub idle_probe_interval: Duration,
    pub active_probe_interval: Duration,
    pub full_recheck_interval: Duration,
    pub max_jitter: Duration,
    pub base_backoff: Duration,
    pub max_backoff: Duration,
    pub resource_cooldown: Duration,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            minimum_idle: Duration::from_secs(5 * 60),
            idle_probe_interval: Duration::from_secs(5),
            active_probe_interval: Duration::from_millis(100),
            full_recheck_interval: Duration::from_secs(2),
            max_jitter: Duration::from_secs(90),
            base_backoff: Duration::from_secs(60),
            max_backoff: Duration::from_secs(60 * 60),
            resource_cooldown: Duration::from_secs(15),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockReason {
    NoInteractiveOwner,
    SessionLocked,
    UserActive,
    BatteryPower,
    BatterySaver,
    ThermalPressure,
    ThermalPressureUnknown,
    MeteredNetwork,
    NetworkCostUnknown,
    Presentation,
    PresentationUnknown,
    Servicing,
    ServicingUnknown,
    MutationActive,
    ReadBudgetBusy,
    Backoff,
    NotDue,
}

impl BlockReason {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NoInteractiveOwner => "noInteractiveOwner",
            Self::SessionLocked => "sessionLocked",
            Self::UserActive => "userActive",
            Self::BatteryPower => "batteryPower",
            Self::BatterySaver => "batterySaver",
            Self::ThermalPressure => "thermalPressure",
            Self::ThermalPressureUnknown => "thermalPressureUnknown",
            Self::MeteredNetwork => "meteredNetwork",
            Self::NetworkCostUnknown => "networkCostUnknown",
            Self::Presentation => "presentation",
            Self::PresentationUnknown => "presentationUnknown",
            Self::Servicing => "servicingBusy",
            Self::ServicingUnknown => "servicingUnknown",
            Self::MutationActive => "mutationActive",
            Self::ReadBudgetBusy => "readBudgetBusy",
            Self::Backoff => "backoff",
            Self::NotDue => "notDue",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunOutcome { Started, Completed, Preempted, Skipped, Failed }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PassiveWorkReport {
    pub evidence_count: u32,
    pub warning_count: u32,
}
