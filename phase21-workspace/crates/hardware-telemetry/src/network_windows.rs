//! Bounded IP Helper reads only. No DNS, ICMP, TCP, HTTP or adapter mutation.
use crate::{
    Result, TelemetryError,
    measurements::{Availability, MAX_NETWORK_ADAPTERS, NetworkAdapter, NetworkCounters},
    network::{CounterWindow, adapter_from_wmi},
};
use aethercore_collector_runtime::CollectorControl;
use std::{
    collections::BTreeMap,
    mem::{align_of, size_of, size_of_val},
    sync::{Mutex, OnceLock},
    time::Instant,
};
use windows::Win32::{
    Foundation::ERROR_BUFFER_OVERFLOW,
    NetworkManagement::IpHelper::*,
    Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN},
};

static SAMPLES: OnceLock<Mutex<CounterWindow>> = OnceLock::new();
static CLOCK: OnceLock<Instant> = OnceLock::new();
fn checkpoint(control: &CollectorControl) -> Result<()> {
    crate::windows_impl::checkpoint(control, "network.iphelper")
}
/// API-owned pointers must be inside our aligned buffer before dereference.
fn contains<T>(buffer: &[u64], pointer: *const T) -> bool {
    let start = buffer.as_ptr() as usize;
    let address = pointer as usize;
    address >= start
        && address.is_multiple_of(align_of::<T>())
        && address
            .checked_add(size_of::<T>())
            .is_some_and(|end| end <= start + size_of_val(buffer))
}
struct Routes(*mut MIB_IPFORWARD_TABLE2);
impl Drop for Routes {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                FreeMibTable(self.0.cast());
            }
        }
    }
}
fn default_routes(control: &CollectorControl) -> Result<BTreeMap<u64, (bool, bool)>> {
    let mut table = Routes(std::ptr::null_mut());
    let status = unsafe { GetIpForwardTable2(AF_UNSPEC, &mut table.0) };
    if status.0 != 0 || table.0.is_null() {
        return Err(TelemetryError::Windows(format!(
            "GetIpForwardTable2:{}",
            status.0
        )));
    }
    let count = unsafe { (*table.0).NumEntries } as usize;
    if count > 4096 {
        return Err(TelemetryError::MalformedResponse(
            "route table exceeds bounded inventory".into(),
        ));
    }
    let mut routes = BTreeMap::new();
    for index in 0..count {
        checkpoint(control)?;
        // The successful API owns count rows; Table is the documented flexible array.
        let row = unsafe { &*(*table.0).Table.as_ptr().add(index) };
        if row.DestinationPrefix.PrefixLength != 0 || row.Loopback {
            continue;
        }
        let family = unsafe { row.DestinationPrefix.Prefix.si_family };
        let flags = routes
            .entry(unsafe { row.InterfaceLuid.Value })
            .or_insert((false, false));
        if family == AF_INET {
            flags.0 = true;
        }
        if family == AF_INET6 {
            flags.1 = true;
        }
    }
    Ok(routes)
}
fn apipa(buffer: &[u64], mut address: *mut IP_ADAPTER_UNICAST_ADDRESS_LH) -> Result<bool> {
    let mut found = false;
    for _ in 0..256 {
        if address.is_null() {
            return Ok(found);
        }
        if !contains(buffer, address) {
            return Err(TelemetryError::MalformedResponse(
                "unicast pointer outside buffer".into(),
            ));
        }
        let item = unsafe { &*address };
        let socket = item.Address.lpSockaddr;
        if item.Address.iSockaddrLength
            < size_of::<windows::Win32::Networking::WinSock::SOCKADDR>() as i32
            || !contains(buffer, socket)
        {
            return Err(TelemetryError::MalformedResponse(
                "socket pointer outside buffer".into(),
            ));
        }
        if unsafe { (*socket).sa_family } == AF_INET {
            let socket = socket.cast::<SOCKADDR_IN>();
            if item.Address.iSockaddrLength < size_of::<SOCKADDR_IN>() as i32
                || !contains(buffer, socket)
            {
                return Err(TelemetryError::MalformedResponse(
                    "truncated IPv4 address".into(),
                ));
            }
            let bytes = unsafe { (*socket).sin_addr.S_un.S_un_b };
            found |= bytes.s_b1 == 169 && bytes.s_b2 == 254;
        }
        address = item.Next;
    }
    Err(TelemetryError::MalformedResponse(
        "unicast list exceeds bound".into(),
    ))
}
pub(crate) fn collect(control: &CollectorControl) -> Result<Vec<NetworkAdapter>> {
    checkpoint(control)?;
    let clock = CLOCK.get_or_init(Instant::now);
    let mut size = 64 * 1024;
    let mut buffer = Vec::<u64>::new();
    let mut success = false;
    for _ in 0..3 {
        if size > 1024 * 1024 {
            return Err(TelemetryError::MalformedResponse(
                "adapter buffer exceeds bound".into(),
            ));
        }
        buffer.resize((size as usize).div_ceil(8), 0);
        let status = unsafe {
            GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER,
                None,
                Some(buffer.as_mut_ptr().cast()),
                &mut size,
            )
        };
        checkpoint(control)?;
        if status == 0 {
            success = true;
            break;
        }
        if status != ERROR_BUFFER_OVERFLOW.0 {
            return Err(TelemetryError::Windows(format!(
                "GetAdaptersAddresses:{status}"
            )));
        }
    }
    if !success {
        return Err(TelemetryError::MalformedResponse(
            "adapter buffer retry exhausted".into(),
        ));
    }
    let routes = default_routes(control);
    checkpoint(control)?;
    let mut pointer = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
    let mut adapters = Vec::new();
    let mut luids = Vec::new();
    for _ in 0..MAX_NETWORK_ADAPTERS {
        if pointer.is_null() {
            break;
        }
        checkpoint(control)?;
        if !contains(&buffer, pointer) {
            return Err(TelemetryError::MalformedResponse(
                "adapter pointer outside buffer".into(),
            ));
        }
        let adapter = unsafe { &*pointer };
        let luid = unsafe { adapter.Luid.Value };
        let mut row = MIB_IF_ROW2 {
            InterfaceLuid: adapter.Luid,
            ..Default::default()
        };
        let read = unsafe { GetIfEntry2(&mut row) }.0 == 0;
        let read_at = u64::try_from(clock.elapsed().as_millis()).unwrap_or(u64::MAX);
        let observed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or_default();
        let name = if read {
            String::from_utf16_lossy(
                &row.Alias[..row
                    .Alias
                    .iter()
                    .position(|v| *v == 0)
                    .unwrap_or(row.Alias.len())],
            )
        } else {
            format!("LUID:{luid:016x}")
        };
        let id = if read {
            format!("{:?}", row.InterfaceGuid)
        } else {
            format!("LUID:{luid:016x}")
        };
        let mut output = adapter_from_wmi(
            &id,
            &name,
            Some(adapter.OperStatus.0 as u32),
            read.then_some(row.MediaConnectState.0 as u32),
            Some(adapter.TransmitLinkSpeed),
            read.then_some(row.InterfaceAndOperStatusFlags._bitfield & 1 == 0),
            observed,
        );
        output.coverage.source = "GetAdaptersAddresses/GetIfEntry2".into();
        output.admin_enabled = match (read, row.AdminStatus.0) {
            (true, 1) => Some(true),
            (true, 2) => Some(false),
            _ => None,
        };
        output.ipv4_apipa = Some(apipa(&buffer, adapter.FirstUnicastAddress)?);
        output.counters = read.then_some(NetworkCounters {
            in_octets: row.InOctets,
            out_octets: row.OutOctets,
            in_errors: row.InErrors,
            out_errors: row.OutErrors,
            in_discards: row.InDiscards,
            out_discards: row.OutDiscards,
        });
        output.counter_availability = if read {
            Availability::Measured
        } else {
            Availability::Failed
        };
        output.route_availability = if routes.is_ok() {
            Availability::Measured
        } else {
            Availability::Failed
        };
        if let Ok(routes) = &routes {
            let flags = routes.get(&luid).copied().unwrap_or_default();
            output.default_route_v4 = Some(flags.0);
            output.default_route_v6 = Some(flags.1);
        }
        adapters.push(output);
        luids.push((luid, read_at));
        pointer = adapter.Next;
    }
    if !pointer.is_null() {
        return Err(TelemetryError::MalformedResponse(
            "adapter inventory exceeds bound or cycles".into(),
        ));
    }
    checkpoint(control)?;
    let mut samples = SAMPLES
        .get_or_init(|| Mutex::new(CounterWindow::default()))
        .lock()
        .map_err(|_| TelemetryError::Windows("counter cache poisoned".into()))?;
    samples.attach_samples(&mut adapters, &luids);
    Ok(adapters)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_buffer_rejects_null_unaligned_and_outside_pointers() {
        let buffer = vec![0u64; 64];
        assert!(contains(&buffer, buffer.as_ptr()));
        assert!(!contains::<u64>(&buffer, std::ptr::null()));
        assert!(!contains(
            &buffer,
            (buffer.as_ptr() as usize + 1) as *const u64
        ));
        assert!(!contains(
            &buffer,
            buffer.as_ptr().wrapping_add(buffer.len())
        ));
    }
    #[test]
    fn apipa_fixture_is_local_only_and_truncated_or_cyclic_records_are_not_readings() {
        let mut buffer = vec![0u64; 64];
        let node = buffer.as_mut_ptr().cast::<IP_ADAPTER_UNICAST_ADDRESS_LH>();
        let socket = unsafe { buffer.as_mut_ptr().add(32) }.cast::<SOCKADDR_IN>();
        unsafe {
            socket.write(SOCKADDR_IN {
                sin_family: AF_INET,
                ..Default::default()
            });
            node.write(IP_ADAPTER_UNICAST_ADDRESS_LH {
                Address: windows::Win32::Networking::WinSock::SOCKET_ADDRESS {
                    lpSockaddr: socket.cast(),
                    iSockaddrLength: size_of::<SOCKADDR_IN>() as i32,
                },
                ..Default::default()
            });
        }
        assert!(!apipa(&buffer, node).unwrap());
        unsafe {
            (*socket).sin_addr.S_un.S_un_b.s_b1 = 169;
            (*socket).sin_addr.S_un.S_un_b.s_b2 = 254;
        }
        assert!(apipa(&buffer, node).unwrap());
        unsafe {
            (*node).Address.iSockaddrLength = 1;
        }
        assert!(apipa(&buffer, node).is_err());
        unsafe {
            (*node).Address.iSockaddrLength = size_of::<SOCKADDR_IN>() as i32;
            (*node).Next = node;
        }
        assert!(apipa(&buffer, node).is_err());
    }
}
