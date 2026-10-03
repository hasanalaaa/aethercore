//! Network adapters (`ROOT\StandardCimv2:MSFT_NetAdapter`), P83-02A.
//!
//! An inventory of what Windows says about each adapter, read locally: no packet is sent, no address
//! is resolved, and no MAC, IP or SSID is collected. A link that is down, an unplugged cable and a
//! virtual adapter are states of an adapter, not statements about "the internet".

use crate::measurements::{
    Availability, Coverage, NetworkAdapter, NetworkCounterDelta, NetworkCounters,
};

#[derive(Default)]
pub(crate) struct CounterWindow(std::collections::BTreeMap<String, (u64, u64, NetworkCounters)>);
impl CounterWindow {
    pub fn observe(
        &mut self,
        id: &str,
        luid: u64,
        now: u64,
        counters: NetworkCounters,
    ) -> Option<NetworkCounterDelta> {
        if !self.0.contains_key(id) && self.0.len() == crate::measurements::MAX_NETWORK_ADAPTERS {
            self.0.pop_first();
        }
        let (old_luid, old_time, old) = self.0.insert(id.into(), (luid, now, counters))?;
        let elapsed_ms = now.checked_sub(old_time).filter(|ms| *ms > 0)?;
        if old_luid != luid {
            return None;
        }
        Some(NetworkCounterDelta {
            elapsed_ms,
            counts: NetworkCounters {
                in_octets: counters.in_octets.checked_sub(old.in_octets)?,
                out_octets: counters.out_octets.checked_sub(old.out_octets)?,
                in_errors: counters.in_errors.checked_sub(old.in_errors)?,
                out_errors: counters.out_errors.checked_sub(old.out_errors)?,
                in_discards: counters.in_discards.checked_sub(old.in_discards)?,
                out_discards: counters.out_discards.checked_sub(old.out_discards)?,
            },
        })
    }
    pub fn retain_present(&mut self, adapters: &[NetworkAdapter]) {
        self.0.retain(|id, _| {
            adapters
                .iter()
                .any(|adapter| adapter.stable_id == *id && adapter.counters.is_some())
        });
    }
}

const SOURCE: &str = "MSFT_NetAdapter";

/// `LinkSpeed` values that mean "not reported": 0 and the all-ones sentinel.
fn link_speed(bits_per_second: Option<u64>) -> Option<u64> {
    bits_per_second.filter(|v| *v != 0 && *v != u64::MAX)
}

/// Builds one adapter from a WMI row. `operational_status` is `IF_OPER_STATUS` (1 up, 2 down, ...,
/// 7 lower layer down); a value outside 1..=7 is not a status. `media_connection_state` is
/// `MediaConnectionState` (1 connected, 2 disconnected); anything else is unknown.
pub(crate) fn adapter_from_wmi(
    interface_guid: &str,
    name: &str,
    operational_status: Option<u32>,
    media_connection_state: Option<u32>,
    link_speed_bps: Option<u64>,
    is_virtual: Option<bool>,
    observed_unix_ms: i64,
) -> NetworkAdapter {
    let operational_status = operational_status
        .filter(|v| (1..=7).contains(v))
        .and_then(|v| u8::try_from(v).ok());
    let connected = match media_connection_state {
        Some(1) => Some(true),
        Some(2) => Some(false),
        _ => None,
    };
    let link_speed_bps = link_speed(link_speed_bps);
    let reported = operational_status.is_some() || connected.is_some() || link_speed_bps.is_some();
    NetworkAdapter {
        stable_id: if interface_guid.is_empty() {
            name.to_string()
        } else {
            interface_guid.to_string()
        },
        display_name: name.to_string(),
        link_speed_bps,
        operational_status,
        connected,
        is_virtual,
        coverage: Coverage {
            source: SOURCE.into(),
            observed_unix_ms: Some(observed_unix_ms),
            window_days: None,
            availability: if reported {
                Availability::Measured
            } else {
                Availability::Unsupported
            },
            reason_key: if reported {
                String::new()
            } else {
                "measurement.reason.noAdapterState".into()
            },
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_window_needs_two_real_samples_and_reset_or_identity_change_never_spikes() {
        let mut window = CounterWindow::default();
        let zero = NetworkCounters::default();
        assert_eq!(window.observe("guid", 1, 100, zero), None);
        let delta = window
            .observe("guid", 1, 1100, zero)
            .expect("measured zero traffic");
        assert_eq!(delta.elapsed_ms, 1000);
        assert_eq!(delta.counts, zero);
        let current = NetworkCounters {
            in_octets: 10,
            out_errors: 2,
            ..zero
        };
        assert_eq!(
            window.observe("guid", 1, 2100, current).unwrap().counts,
            current
        );
        assert_eq!(
            window.observe("guid", 1, 2100, current),
            None,
            "no real elapsed window"
        );
        assert_eq!(window.observe("guid", 1, 2200, zero), None, "counter reset");
        assert_eq!(
            window.observe("guid", 2, 2300, current),
            None,
            "LUID changed"
        );
        assert_eq!(
            window.observe("other-guid", 2, 2400, current),
            None,
            "GUID changed"
        );
        window.retain_present(&[]);
        assert_eq!(
            window.observe("guid", 1, 2500, current),
            None,
            "hot unplug drops the baseline"
        );
        for id in 0..1000 {
            window.observe(&id.to_string(), id, 3000, zero);
        }
        assert_eq!(window.0.len(), crate::measurements::MAX_NETWORK_ADAPTERS);
    }

    #[test]
    fn a_reported_state_is_kept_as_reported() {
        let a = adapter_from_wmi(
            "{G}",
            "Ethernet",
            Some(1),
            Some(1),
            Some(1_000_000_000),
            Some(false),
            5,
        );
        assert_eq!(
            (
                a.operational_status,
                a.connected,
                a.link_speed_bps,
                a.is_virtual
            ),
            (Some(1), Some(true), Some(1_000_000_000), Some(false))
        );
        assert_eq!(a.stable_id, "{G}");
        assert_eq!(a.coverage.availability, Availability::Measured);
    }

    #[test]
    fn a_down_adapter_an_unplugged_cable_and_a_virtual_one_are_states_not_faults() {
        let down = adapter_from_wmi("{G}", "Wi-Fi", Some(2), None, None, Some(false), 5);
        assert_eq!(down.operational_status, Some(2), "down is a status");
        assert_eq!(
            down.connected, None,
            "an unreported media state is unknown, not disconnected"
        );
        let unplugged = adapter_from_wmi("{G}", "Ethernet", Some(2), Some(2), None, Some(false), 5);
        assert_eq!(unplugged.connected, Some(false));
        let vpn = adapter_from_wmi(
            "{G}",
            "VPN",
            Some(1),
            Some(1),
            Some(10_000_000),
            Some(true),
            5,
        );
        assert_eq!(
            vpn.is_virtual,
            Some(true),
            "a virtual adapter is named as such and classified by nothing else"
        );
        assert_eq!(vpn.coverage.availability, Availability::Measured);
    }

    #[test]
    fn unreported_values_are_absent_and_an_adapter_that_reports_nothing_says_so() {
        let a = adapter_from_wmi("", "Bluetooth", Some(99), Some(7), Some(u64::MAX), None, 5);
        assert_eq!(
            (a.operational_status, a.connected, a.link_speed_bps),
            (None, None, None),
            "out of range, unknown code, all-ones sentinel"
        );
        assert_eq!(a.stable_id, "Bluetooth", "no GUID: the name identifies it");
        assert_eq!(a.coverage.availability, Availability::Unsupported);
        assert_eq!(a.coverage.reason_key, "measurement.reason.noAdapterState");
        assert_eq!(
            adapter_from_wmi("{G}", "X", None, None, Some(0), None, 5).link_speed_bps,
            None,
            "0 bit/s is not a speed"
        );
    }
}
