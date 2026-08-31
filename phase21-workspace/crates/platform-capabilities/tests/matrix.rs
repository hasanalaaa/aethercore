//! Phase 26 — capability matrix pinning tests (cfg-gated per OS; all run on this host).

use aethercore_platform_capabilities::{
    Availability, Platform, PlatformCapability as C, WindowsSku, available_on,
    available_on_windows_sku, classify_windows_sku, keys, matrix_for_current_platform,
};

fn is_na(av: &Availability) -> bool {
    matches!(av, Availability::NotAvailable { .. })
}

fn is_native(av: &Availability) -> bool {
    matches!(av, Availability::Native)
}

// --- FROZEN Windows table ------------------------------------------------------

#[test]
fn windows_matrix_is_all_native_frozen() {
    for cap in C::ALL {
        assert!(
            is_native(&available_on(Platform::Windows, *cap)),
            "Windows freeze broken for {cap:?}"
        );
    }
}

// --- macOS table (this host's OS) ----------------------------------------------

#[cfg(target_os = "macos")]
#[test]
fn macos_telemetry_timeline_intelligence_care_available() {
    let native = [
        C::TelemetryCpu,
        C::TelemetryMemory,
        C::TelemetryStorage,
        C::TimelineIntelligence,
        C::LocalIntelligence,
        C::CareOrchestration,
    ];
    for cap in native {
        assert!(
            !is_na(&available_on(Platform::Macos, cap)),
            "{cap:?} must not be NotAvailable on macOS"
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn macos_windows_exclusive_capabilities_are_not_available_with_typed_reasons() {
    let cases = [
        (C::DriverServicing, keys::MACOS_NO_DRIVER_STORE),
        (C::SystemRepairDism, keys::MACOS_NO_DISM_SFC_WUA),
        (C::SystemRepairSfc, keys::MACOS_NO_DISM_SFC_WUA),
        (C::SystemRepairWua, keys::MACOS_NO_DISM_SFC_WUA),
        (C::ProcessGovernorEcoQos, keys::MACOS_NO_ECOQOS),
        (C::GameModeProfile, keys::WINDOWS_ONLY_API),
        (C::RestorePoints, keys::MACOS_RESTORE_NA),
        (C::WindowsUpdate, keys::MACOS_NO_WINUPDATE),
    ];
    for (cap, reason) in cases {
        match available_on(Platform::Macos, cap) {
            Availability::NotAvailable { reason_key } => {
                assert_eq!(reason_key, reason, "wrong reason for {cap:?}");
            }
            other => panic!("{cap:?} must be NotAvailable on macOS, got {other:?}"),
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn macos_gpu_degraded_and_thermal_degraded() {
    match available_on(Platform::Macos, C::TelemetryGpu) {
        Availability::Degraded { note_key } => {
            assert_eq!(note_key, keys::MACOS_GPU_LIMITED)
        }
        other => panic!("gpu should be Degraded on macOS, got {other:?}"),
    }
    match available_on(Platform::Macos, C::ThermalPowerClamp) {
        Availability::Degraded { note_key } => {
            assert_eq!(note_key, keys::MACOS_THERMAL_NQ)
        }
        other => panic!("thermal should be Degraded on macOS, got {other:?}"),
    }
}

// --- Cross-platform invariants --------------------------------------------------

#[test]
fn every_platform_answers_every_capability() {
    for platform in [Platform::Windows, Platform::Macos, Platform::Linux] {
        for cap in C::ALL {
            // Must return a value (never panic) for the full cross product.
            let _ = available_on(platform, *cap);
        }
    }
}

#[test]
fn current_platform_matrix_is_complete_and_non_empty() {
    let matrix = matrix_for_current_platform();
    assert_eq!(matrix.len(), C::ALL.len());
    assert!(matrix.iter().all(|(name, _)| !name.is_empty()));
}

#[test]
#[cfg(target_os = "macos")]
fn current_platform_is_macos_on_this_host() {
    assert_eq!(Platform::current(), Platform::Macos);
}

#[test]
fn capability_names_are_unique() {
    let mut names: Vec<&'static str> = C::ALL.iter().map(|c| c.as_str()).collect();
    names.sort_unstable();
    let len = names.len();
    names.dedup();
    assert_eq!(names.len(), len, "duplicate capability name");
}

#[test]
fn windows_sku_classification_distinguishes_server_core() {
    assert_eq!(classify_windows_sku(1, "Client"), WindowsSku::Workstation);
    assert_eq!(classify_windows_sku(3, "Server"), WindowsSku::Server);
    assert_eq!(classify_windows_sku(3, "Server Core"), WindowsSku::ServerCore);
    assert_eq!(classify_windows_sku(99, ""), WindowsSku::Unknown);
}

#[test]
fn server_matrix_reports_client_only_surfaces_honestly() {
    for sku in [WindowsSku::Server, WindowsSku::ServerCore] {
        assert!(matches!(
            available_on_windows_sku(sku, C::ThermalPowerClamp),
            Availability::NotAvailable { .. }
        ));
        assert!(matches!(
            available_on_windows_sku(sku, C::GameModeProfile),
            Availability::NotAvailable { .. }
        ));
        assert!(matches!(
            available_on_windows_sku(sku, C::RestorePoints),
            Availability::NotAvailable { .. }
        ));
    }
    assert!(matches!(
        available_on_windows_sku(WindowsSku::Server, C::WindowsUpdate),
        Availability::Degraded { .. }
    ));
    assert!(matches!(
        available_on_windows_sku(WindowsSku::ServerCore, C::CareOrchestration),
        Availability::Degraded { .. }
    ));
}
