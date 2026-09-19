use std::{
    ffi::c_void,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use aethercore_security::inspect_session_token;
use aethercore_windows_foundation::{
    BackgroundThreadMode, ComApartment, OwnedHandle, OwnedServiceHandle, ThreadImpersonation,
};
use chrono::Utc;
use windows::{
    Win32::{
        Foundation::HANDLE,
        Networking::NetworkListManager::{
            INetworkCostManager, NLM_CONNECTION_COST_APPROACHINGDATALIMIT,
            NLM_CONNECTION_COST_CONGESTED, NLM_CONNECTION_COST_FIXED,
            NLM_CONNECTION_COST_OVERDATALIMIT, NLM_CONNECTION_COST_ROAMING,
            NLM_CONNECTION_COST_VARIABLE,
        },
        System::{
            Com::{
                CLSCTX_ALL, CLSCTX_INPROC_SERVER, CoCreateInstance, CoSetProxyBlanket, EOAC_NONE,
                RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE,
            },
            Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
            RemoteDesktop::{
                WTSActive, WTSFreeMemory, WTSGetActiveConsoleSessionId, WTSINFOEXW,
                WTSQuerySessionInformationW, WTSQueryUserToken, WTSSessionInfoEx,
            },
            Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE},
            Services::{
                OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SC_MANAGER_CONNECT,
                SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS, SERVICE_RUNNING,
                SERVICE_STATUS_PROCESS,
            },
            Variant::VARIANT,
            Wmi::{
                IWbemClassObject, IWbemLocator, IWbemServices, WBEM_FLAG_FORWARD_ONLY,
                WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_S_FALSE, WBEM_S_TIMEDOUT, WbemLocator,
            },
        },
        UI::Shell::{
            QUNS_ACCEPTS_NOTIFICATIONS, QUNS_BUSY, QUNS_NOT_PRESENT, QUNS_PRESENTATION_MODE,
            QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
        },
    },
    core::{BSTR, GUID, PCWSTR, PWSTR},
};

use crate::{
    NetworkCost, PresentationState, ServicingState, SystemState, SystemStateProbe, ThermalPressure,
};

const NO_CONSOLE_SESSION: u32 = 0xffff_ffff;
const FILETIME_TICKS_PER_SECOND: i64 = 10_000_000;
const NETWORK_LIST_MANAGER_CLSID: GUID = GUID::from_u128(0xdcb00c01_570f_4a9b_8d69_199fdba5723b);

#[derive(Clone, Copy)]
struct SlowSignals {
    sampled: Instant,
    network: NetworkCost,
    servicing: ServicingState,
    thermal: ThermalPressure,
}

pub struct WindowsSystemStateProbe {
    slow: Arc<Mutex<Option<SlowSignals>>>,
    slow_refreshing: Arc<AtomicBool>,
}

impl Default for WindowsSystemStateProbe {
    fn default() -> Self {
        Self {
            slow: Arc::new(Mutex::new(None)),
            slow_refreshing: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl WindowsSystemStateProbe {
    pub fn new() -> Self {
        Self::default()
    }

    fn slow_signals(&self, refresh: bool) -> SlowSignals {
        let mut cache = self.slow.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(value) = *cache {
            if !refresh || value.sampled.elapsed() < Duration::from_secs(2) {
                return value;
            }
        } else if !refresh {
            return SlowSignals {
                sampled: Instant::now(),
                network: NetworkCost::Unknown,
                servicing: ServicingState::Unknown,
                thermal: ThermalPressure::Unknown,
            };
        }
        let value = collect_slow_signals();
        *cache = Some(value);
        value
    }

    fn refresh_slow_nonblocking_impl(&self) {
        if self
            .slow_refreshing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let slow = self.slow.clone();
        let refreshing = self.slow_refreshing.clone();
        let spawn = thread::Builder::new()
            .name("aether-idle-slow-signals".into())
            .spawn(move || {
                let value = catch_unwind(AssertUnwindSafe(|| {
                    run_in_background_mode(|| Ok(collect_slow_signals()))
                }))
                .ok()
                .and_then(Result::ok);
                if let Some(value) = value {
                    *slow.lock().unwrap_or_else(|p| p.into_inner()) = Some(value);
                }
                refreshing.store(false, Ordering::Release);
            });
        if spawn.is_err() {
            self.slow_refreshing.store(false, Ordering::Release);
        }
    }
}

impl WindowsSystemStateProbe {
    fn sample_impl(&self, refresh_slow: bool) -> Result<SystemState, String> {
        unsafe {
            let session_id = WTSGetActiveConsoleSessionId();
            if session_id == NO_CONSOLE_SESSION {
                return Err("no active console session".into());
            }

            let mut raw_token = HANDLE::default();
            WTSQueryUserToken(session_id, &mut raw_token)
                .map_err(|e| format!("WTSQueryUserToken: {e}"))?;
            let token = OwnedHandle::new(raw_token);
            (|| {
                let principal =
                    inspect_session_token(token.get(), session_id).map_err(|e| e.to_string())?;
                let (idle_for, active, unlocked) = query_idle(session_id)?;
                let presentation = query_presentation_as_user(token.get())?;
                let mut power = SYSTEM_POWER_STATUS::default();
                GetSystemPowerStatus(&mut power)
                    .map_err(|e| format!("GetSystemPowerStatus: {e}"))?;
                let slow = self.slow_signals(refresh_slow);
                let session_unlocked = active
                    && unlocked
                    && !matches!(presentation, PresentationState::LockedOrAbsent);
                Ok(SystemState {
                    sampled_unix_ms: Utc::now().timestamp_millis(),
                    owner_principal_key: principal.binding_key(),
                    user_sid: principal.user_sid,
                    session_id,
                    idle_for,
                    session_unlocked,
                    on_ac_power: power.ACLineStatus == 1,
                    battery_saver: power.SystemStatusFlag != 0,
                    thermal_pressure: slow.thermal,
                    network_cost: slow.network,
                    presentation,
                    servicing: slow.servicing,
                })
            })()
        }
    }
}

impl SystemStateProbe for WindowsSystemStateProbe {
    fn sample(&self) -> Result<SystemState, String> {
        self.sample_impl(true)
    }
    fn sample_fast(&self) -> Result<SystemState, String> {
        self.sample_impl(false)
    }
    fn refresh_slow_nonblocking(&self) {
        self.refresh_slow_nonblocking_impl();
    }
}

fn query_idle(session_id: u32) -> Result<(Duration, bool, bool), String> {
    unsafe {
        let mut raw = PWSTR::null();
        let mut bytes = 0u32;
        WTSQuerySessionInformationW(None, session_id, WTSSessionInfoEx, &mut raw, &mut bytes)
            .map_err(|e| format!("WTSSessionInfoEx: {e}"))?;
        if raw.is_null() {
            return Err("WTSSessionInfoEx returned a null buffer".into());
        }
        let result = (|| {
            if bytes < size_of::<WTSINFOEXW>() as u32 {
                return Err("WTSSessionInfoEx returned a truncated buffer".into());
            }
            let info = &*(raw.0.cast::<WTSINFOEXW>());
            if info.Level != 1 {
                return Err("WTSSessionInfoEx returned an unsupported level".into());
            }
            let level = info.Data.WTSInfoExLevel1;
            if level.SessionId != session_id {
                return Err("WTSSessionInfoEx session id mismatch".into());
            }
            let ticks = level.CurrentTime.saturating_sub(level.LastInputTime).max(0);
            // On supported Windows 10/11 targets, SessionFlags=1 means unlocked and 0 means locked.
            // Treat every other value fail-closed as not unlocked.
            let unlocked = level.SessionFlags == 1;
            Ok((
                Duration::from_secs((ticks / FILETIME_TICKS_PER_SECOND) as u64),
                level.SessionState == WTSActive,
                unlocked,
            ))
        })();
        WTSFreeMemory(raw.0.cast::<c_void>());
        result
    }
}

fn query_presentation_as_user(token: HANDLE) -> Result<PresentationState, String> {
    unsafe {
        let impersonation = ThreadImpersonation::logged_on_user(token)
            .map_err(|e| format!("ImpersonateLoggedOnUser: {e}"))?;
        let value = SHQueryUserNotificationState();
        // SECURITY INVARIANT: the RAII guard provides a fallback revert on early return, while the
        // explicit revert remains observable and security-fatal if Windows refuses to drop context.
        impersonation.revert().map_err(|error| {
            format!("security-fatal: RevertToSelf failed in idle scheduler probe: {error}")
        })?;
        Ok(match value {
            Ok(v) if v == QUNS_ACCEPTS_NOTIFICATIONS => PresentationState::Clear,
            Ok(v) if v == QUNS_BUSY => PresentationState::Busy,
            Ok(v) if v == QUNS_RUNNING_D3D_FULL_SCREEN => PresentationState::FullScreen,
            Ok(v) if v == QUNS_PRESENTATION_MODE => PresentationState::Presentation,
            Ok(v) if v == QUNS_NOT_PRESENT => PresentationState::LockedOrAbsent,
            Ok(_) | Err(_) => PresentationState::Unknown,
        })
    }
}

fn collect_slow_signals() -> SlowSignals {
    SlowSignals {
        sampled: Instant::now(),
        network: probe_network_cost().unwrap_or(NetworkCost::Unknown),
        servicing: probe_servicing_state().unwrap_or(ServicingState::Unknown),
        // ACPI thermal-zone telemetry is optional. Preserve Unknown when the platform does not
        // expose trustworthy current + critical trip-point evidence.
        thermal: probe_thermal_pressure().unwrap_or(ThermalPressure::Unknown),
    }
}

const THERMAL_WMI_DEADLINE: Duration = Duration::from_millis(1_200);
const THERMAL_WMI_SLICE_MS: i32 = 200;
const MAX_THERMAL_ZONES: usize = 16;

fn probe_thermal_pressure() -> Result<ThermalPressure, String> {
    unsafe {
        let _com = ComApartment::mta()
            .map_err(|hr| format!("CoInitializeEx thermal probe failed: 0x{:08X}", hr.0 as u32))?;
        (|| {
            let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("thermal WbemLocator: {e}"))?;
            let services: IWbemServices = locator
                .ConnectServer(
                    &BSTR::from("ROOT\\WMI"),
                    &BSTR::new(),
                    &BSTR::new(),
                    &BSTR::new(),
                    0,
                    &BSTR::new(),
                    None,
                )
                .map_err(|e| format!("thermal ConnectServer: {e}"))?;
            CoSetProxyBlanket(
                &services,
                RPC_C_AUTHN_WINNT,
                RPC_C_AUTHZ_NONE,
                PCWSTR::null(),
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            )
            .map_err(|e| format!("thermal CoSetProxyBlanket: {e}"))?;
            let flags = WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY;
            let enumerator = services.ExecQuery(
                &BSTR::from("WQL"),
                &BSTR::from("SELECT CurrentTemperature,CriticalTripPoint FROM MSAcpi_ThermalZoneTemperature"),
                flags,
                None,
            ).map_err(|e| format!("thermal ExecQuery: {e}"))?;

            let deadline = Instant::now() + THERMAL_WMI_DEADLINE;
            let mut worst = ThermalPressure::Unknown;
            let mut zones = 0usize;
            loop {
                if Instant::now() >= deadline {
                    return Err("thermal WMI enumeration deadline exceeded".into());
                }
                let mut values: [Option<IWbemClassObject>; 1] = [None];
                let mut returned = 0u32;
                let status = enumerator.Next(THERMAL_WMI_SLICE_MS, &mut values, &mut returned);
                if status.0 == WBEM_S_FALSE.0 {
                    if returned != 0 || values[0].is_some() {
                        return Err("thermal WMI returned terminal WBEM_S_FALSE with data".into());
                    }
                    break;
                }
                if status.is_err() {
                    return Err(format!(
                        "thermal WMI Next failed: 0x{:08X}",
                        status.0 as u32
                    ));
                }
                if returned > 1 {
                    return Err("thermal WMI returned more objects than the supplied slice".into());
                }
                match (returned, values[0].take()) {
                    (0, None) if status.0 == WBEM_S_TIMEDOUT.0 => continue,
                    (0, None) => {
                        return Err(
                            "thermal WMI returned zero objects without timeout/end status".into(),
                        );
                    }
                    (1, Some(object)) => {
                        zones += 1;
                        if zones > MAX_THERMAL_ZONES {
                            return Err("thermal WMI exceeded bounded zone count".into());
                        }
                        let current = thermal_prop_u32(&object, "CurrentTemperature");
                        let critical = thermal_prop_u32(&object, "CriticalTripPoint");
                        if let (Some(current), Some(critical)) = (current, critical) {
                            let zone = classify_thermal_zone(current, critical);
                            worst = max_thermal_pressure(worst, zone);
                        }
                    }
                    (0, Some(_)) => {
                        return Err(
                            "thermal WMI populated an object while reporting zero returned".into(),
                        );
                    }
                    (1, None) => {
                        return Err("thermal WMI reported one object without supplying it".into());
                    }
                    _ => return Err("thermal WMI returned an invalid object count".into()),
                }
            }
            Ok(worst)
        })()
    }
}

fn max_thermal_pressure(a: ThermalPressure, b: ThermalPressure) -> ThermalPressure {
    fn rank(v: ThermalPressure) -> u8 {
        match v {
            ThermalPressure::Unknown => 0,
            ThermalPressure::Normal => 1,
            ThermalPressure::Elevated => 2,
            ThermalPressure::Critical => 3,
        }
    }
    if rank(b) > rank(a) { b } else { a }
}

fn classify_thermal_zone(
    current_tenths_kelvin: u32,
    critical_tenths_kelvin: u32,
) -> ThermalPressure {
    // Reject nonsensical/absent trip points rather than deriving arbitrary temperature thresholds.
    if !(2_000..=5_000).contains(&current_tenths_kelvin)
        || !(2_000..=5_000).contains(&critical_tenths_kelvin)
        || critical_tenths_kelvin <= current_tenths_kelvin.saturating_sub(500)
    {
        return ThermalPressure::Unknown;
    }
    let margin = critical_tenths_kelvin.saturating_sub(current_tenths_kelvin);
    if margin <= 50 {
        ThermalPressure::Critical
    } else if margin <= 150 {
        ThermalPressure::Elevated
    } else {
        ThermalPressure::Normal
    }
}

fn thermal_prop_u32(object: &IWbemClassObject, name: &str) -> Option<u32> {
    unsafe {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut value = VARIANT::default();
        object
            .Get(PCWSTR(wide.as_ptr()), 0, &mut value, None, None)
            .ok()?;
        u32::try_from(&value)
            .ok()
            .or_else(|| u16::try_from(&value).ok().map(u32::from))
    }
}

fn probe_network_cost() -> Result<NetworkCost, String> {
    unsafe {
        let _com = ComApartment::mta()
            .map_err(|hr| format!("CoInitializeEx network cost failed: 0x{:08X}", hr.0 as u32))?;
        (|| {
            let manager: INetworkCostManager =
                CoCreateInstance(&NETWORK_LIST_MANAGER_CLSID, None, CLSCTX_ALL)
                    .map_err(|e| format!("NetworkListManager: {e}"))?;
            let mut cost = 0u32;
            manager
                .GetCost(&mut cost, ptr::null())
                .map_err(|e| format!("INetworkCostManager::GetCost: {e}"))?;
            let metered_mask = NLM_CONNECTION_COST_FIXED.0 as u32
                | NLM_CONNECTION_COST_VARIABLE.0 as u32
                | NLM_CONNECTION_COST_OVERDATALIMIT.0 as u32
                | NLM_CONNECTION_COST_CONGESTED.0 as u32
                | NLM_CONNECTION_COST_ROAMING.0 as u32
                | NLM_CONNECTION_COST_APPROACHINGDATALIMIT.0 as u32;
            Ok(if cost & metered_mask != 0 {
                NetworkCost::Metered
            } else {
                NetworkCost::Unmetered
            })
        })()
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn probe_servicing_state() -> Result<ServicingState, String> {
    unsafe {
        let scm = OwnedServiceHandle::new(
            OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT)
                .map_err(|e| format!("OpenSCManagerW: {e}"))?,
        );
        let mut observed = 0usize;
        let mut uncertain = false;
        for name in ["TrustedInstaller", "UsoSvc", "WaaSMedicSvc"] {
            let name_w = wide(name);
            let service =
                match OpenServiceW(scm.get(), PCWSTR(name_w.as_ptr()), SERVICE_QUERY_STATUS) {
                    Ok(value) => OwnedServiceHandle::new(value),
                    Err(_) => continue,
                };
            let mut status = SERVICE_STATUS_PROCESS::default();
            let mut needed = 0u32;
            let bytes = std::slice::from_raw_parts_mut(
                (&mut status as *mut SERVICE_STATUS_PROCESS).cast::<u8>(),
                size_of::<SERVICE_STATUS_PROCESS>(),
            );
            if QueryServiceStatusEx(
                service.get(),
                SC_STATUS_PROCESS_INFO,
                Some(bytes),
                &mut needed,
            )
            .is_err()
            {
                uncertain = true;
                continue;
            }
            observed += 1;
            if status.dwCurrentState == SERVICE_RUNNING {
                return Ok(ServicingState::Busy);
            }
        }
        Ok(if observed == 0 || uncertain {
            ServicingState::Unknown
        } else {
            ServicingState::Idle
        })
    }
}

pub(crate) fn run_in_background_mode<T>(
    f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _background = BackgroundThreadMode::enter()
        .map_err(|error| format!("THREAD_MODE_BACKGROUND_BEGIN failed: {error}"))?;
    f()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thermal_zone_uses_trip_point_margin_without_invented_absolute_thresholds() {
        assert_eq!(classify_thermal_zone(3_000, 3_300), ThermalPressure::Normal);
        assert_eq!(
            classify_thermal_zone(3_200, 3_300),
            ThermalPressure::Elevated
        );
        assert_eq!(
            classify_thermal_zone(3_260, 3_300),
            ThermalPressure::Critical
        );
        assert_eq!(classify_thermal_zone(0, 0), ThermalPressure::Unknown);
    }
    #[test]
    fn worst_thermal_zone_wins() {
        assert_eq!(
            max_thermal_pressure(ThermalPressure::Normal, ThermalPressure::Elevated),
            ThermalPressure::Elevated
        );
        assert_eq!(
            max_thermal_pressure(ThermalPressure::Critical, ThermalPressure::Normal),
            ThermalPressure::Critical
        );
    }

    /// Windows-lab probe only. This exercises state observation and never starts a workload or
    /// mutates machine state. It is ignored in normal CI because eligibility depends on the live
    /// console session, power policy, presentation state, network and servicing providers.
    #[test]
    #[ignore = "read-only live Phase 14 eligibility probe"]
    fn live_read_only_system_state_probe() {
        let probe = WindowsSystemStateProbe::new();
        let _ = probe.sample();
        probe.refresh_slow_nonblocking();
        std::thread::sleep(Duration::from_millis(250));
        let _ = probe.sample_fast();
    }
}
