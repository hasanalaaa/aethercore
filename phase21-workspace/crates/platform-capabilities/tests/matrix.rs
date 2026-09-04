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

/// DBT-P46-B26. This is the single canonical Windows-SKU decider and it gates
/// capability availability, so a registry read that failed must not be able to
/// answer it. `InstallationType` is exactly what separates Server from Server
/// Core, and Server Core is the SKU with no console — the one thing the two
/// server tables actually disagree about.
#[test]
fn an_unread_installation_type_cannot_answer_the_sku() {
    // The value does not participate in a workstation's classification, so a
    // failed read there must not degrade a correct answer.
    assert_eq!(classify_windows_sku(1, None), WindowsSku::Workstation);

    // It is the whole basis of the server split, so it must not be guessed.
    assert_eq!(classify_windows_sku(3, None), WindowsSku::Unknown);
    assert_eq!(classify_windows_sku(2, None), WindowsSku::Unknown);

    // And an unknown SKU must not claim the one capability Server Core lacks.
    assert!(
        matches!(
            available_on_windows_sku(WindowsSku::Unknown, C::CareOrchestration),
            Availability::Degraded { .. }
        ),
        "Unknown cannot rule out Server Core, so it must not claim a console"
    );
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

/// Regression test for the SKU-detection defect found in Phase 38.
///
/// `current_windows_sku()` read a `ProductType` **DWORD** from
/// `SOFTWARE\Microsoft\Windows NT\CurrentVersion`. That value does not exist on
/// Windows -- measured ABSENT on Windows 11 Pro build 26200 -- so the read
/// always failed and the function returned `Unknown` on EVERY Windows host.
/// Consequences observed on a real install of 0.1.7: `aetherctl about` reported
/// `"platform":"windowsUnknownSku"` instead of `"windows"`, and the capability
/// matrix fell back to the SERVER table on a workstation.
///
/// The numeric product type actually lives in ProductOptions as a REG_SZ. This
/// pins that mapping, and it is discriminating on every host because it tests
/// the pure function rather than the registry.
#[test]
fn product_type_code_maps_the_documented_registry_strings() {
    use aethercore_platform_capabilities::{
        classify_windows_sku, product_type_code, WindowsSku,
    };

    // The documented ProductOptions\ProductType values, and their VER_NT_* codes.
    assert_eq!(product_type_code("WinNT"), 1, "workstation");
    assert_eq!(product_type_code("LanmanNT"), 2, "domain controller");
    assert_eq!(product_type_code("ServerNT"), 3, "server");

    // Windows writes these with this exact casing, but the registry is not
    // case-sensitive and neither is the mapping.
    assert_eq!(product_type_code("winnt"), 1);
    assert_eq!(product_type_code("SERVERNT"), 3);

    // Fail closed: anything unrecognised must NOT be claimed as a workstation.
    assert_eq!(product_type_code(""), 0);
    assert_eq!(product_type_code("Whatever"), 0);
    assert_eq!(classify_windows_sku(product_type_code(""), ""), WindowsSku::Unknown);

    // End to end, the combination this box actually reports:
    // ProductOptions\ProductType = "WinNT", CurrentVersion\InstallationType = "Client".
    assert_eq!(
        classify_windows_sku(product_type_code("WinNT"), "Client"),
        WindowsSku::Workstation,
        "a Windows workstation must classify as Workstation, not Unknown"
    );
    // And the Server / Server Core split still works.
    assert_eq!(
        classify_windows_sku(product_type_code("ServerNT"), "Server"),
        WindowsSku::Server
    );
    assert_eq!(
        classify_windows_sku(product_type_code("ServerNT"), "Server Core"),
        WindowsSku::ServerCore
    );
}

// ---------------------------------------------------------------------------
// P42 — capabilities may not contradict the collectors (§20.1.1 site 9)
// ---------------------------------------------------------------------------

/// The durable form of the DBT-P41-002 test 4. The integration test in
/// `performance-telemetry` asserts the same property against *this* machine's
/// real collectors, which passes whenever the hardware is healthy — exactly the
/// §20.1.6 "gpu got it right by accident" trap. This one holds the property
/// under a hostile observation regardless of the host.
#[test]
fn an_unmeasured_telemetry_subsystem_is_never_reported_native() {
    use aethercore_platform_capabilities::{
        TelemetryObservation, matrix_for_current_platform_observed,
    };

    let nothing_measured = matrix_for_current_platform_observed(TelemetryObservation::UNOBSERVED);
    for name in [
        "telemetryCpu",
        "telemetryMemory",
        "telemetryStorage",
        "telemetryGpu",
    ] {
        let (_, availability) = nothing_measured
            .iter()
            .find(|(n, _)| *n == name)
            .unwrap_or_else(|| panic!("{name} missing from the matrix"));
        assert!(
            !is_native(availability),
            "{name} reported `native` while its collector measured nothing: {availability:?}"
        );
    }
}

/// One unmeasured subsystem must not drag down the others, and non-telemetry
/// capabilities are untouched by a telemetry observation.
#[test]
fn observation_downgrades_only_the_subsystem_that_reported_nothing() {
    use aethercore_platform_capabilities::{
        TelemetryObservation, matrix_for_current_platform, matrix_for_current_platform_observed,
    };

    let observed = matrix_for_current_platform_observed(TelemetryObservation {
        cpu: true,
        memory: true,
        storage: false,
        gpu: true,
    });
    let platform_shape = matrix_for_current_platform();
    for (name, availability) in &observed {
        let (_, unobserved) = platform_shape
            .iter()
            .find(|(n, _)| n == name)
            .expect("same capability set");
        if *name == "telemetryStorage" {
            assert!(
                !is_native(availability),
                "storage measured nothing but still reports {availability:?}"
            );
        } else {
            assert_eq!(
                availability, unobserved,
                "{name} changed on an observation that did not concern it"
            );
        }
    }
}

/// An observation may only ever downgrade. A platform that does not offer a
/// capability must not be promoted by a collector claiming to have measured it.
#[test]
fn an_observation_never_promotes_a_capability() {
    use aethercore_platform_capabilities::{
        TelemetryObservation, matrix_for_current_platform, matrix_for_current_platform_observed,
    };

    let all_measured = matrix_for_current_platform_observed(TelemetryObservation {
        cpu: true,
        memory: true,
        storage: true,
        gpu: true,
    });
    assert_eq!(
        all_measured,
        matrix_for_current_platform(),
        "a fully-measured observation must leave the platform matrix unchanged"
    );
}
