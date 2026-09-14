//! Platform capabilities and the honest engine-source label.

use super::*;

pub(super) fn get_platform_capabilities() -> Routed {
    let capabilities = aethercore_platform_capabilities::matrix_for_current_platform_observed(
        crate::performance::observe_telemetry(),
    )
    .into_iter()
    .map(|(name, availability)| {
        let state = match &availability {
            aethercore_platform_capabilities::Availability::Native => "native",
            aethercore_platform_capabilities::Availability::Degraded { .. } => "degraded",
            aethercore_platform_capabilities::Availability::NotAvailable { .. } => "notAvailable",
        };
        let key = match availability {
            aethercore_platform_capabilities::Availability::Native => String::new(),
            aethercore_platform_capabilities::Availability::Degraded { note_key } => {
                note_key.to_string()
            }
            aethercore_platform_capabilities::Availability::NotAvailable { reason_key } => {
                reason_key.to_string()
            }
        };
        v1::PlatformCapabilityStatus {
            name: name.to_string(),
            availability: Some(v1::CapabilityAvailability {
                state: state.to_string(),
                key,
            }),
        }
    })
    .collect();
    Ok(Some(response::Payload::PlatformCapabilitiesResponse(
        v1::PlatformCapabilitiesResponse {
            platform: aethercore_platform_capabilities::current_platform_name().to_string(),
            capabilities,
        },
    )))
}

pub(super) fn get_engine_source() -> Routed {
    Ok(Some(response::Payload::EngineSourceResponse(
        v1::EngineSourceResponse {
            source: crate::performance::engine_source().to_string(),
            platform: aethercore_platform_capabilities::current_platform_name().to_string(),
        },
    )))
}
