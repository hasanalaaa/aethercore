use std::{collections::HashMap, mem::{offset_of, size_of}, sync::OnceLock};

use aethercore_collector_runtime::{
    CancellationToken, CollectorControl, CollectorFault, CollectorFaultRecord, FaultKind, IsolationGate, DEFAULT_COLLECTOR_TIMEOUT,
    STORAGE_IOCTL_TIMEOUT, WMI_NEXT_SLICE, run_isolated_gated_with_token,
};
use aethercore_restore_point::initialize_process_com_security;
use aethercore_windows_foundation::{ComApartment, OwnedHandle};
use windows::{
    Win32::{
        Foundation::E_ACCESSDENIED,
        Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING},
        System::{
            Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoSetProxyBlanket, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE},
            IO::DeviceIoControl,
            Ioctl::{IDEREGS, IOCTL_STORAGE_QUERY_PROPERTY, READ_ATTRIBUTES, SENDCMDINPARAMS, SENDCMDOUTPARAMS, SMART_CMD, SMART_CYL_HI, SMART_CYL_LOW, SMART_RCV_DRIVE_DATA, STORAGE_PROPERTY_QUERY, STORAGE_PROTOCOL_DATA_DESCRIPTOR, STORAGE_PROTOCOL_SPECIFIC_DATA, StorageDeviceProtocolSpecificProperty, PropertyStandardQuery, ProtocolTypeNvme},
            Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE},
            SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
            Variant::VARIANT,
            Wmi::{IWbemLocator, IWbemServices, WBEM_E_ACCESS_DENIED, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_S_FALSE, WBEM_S_TIMEDOUT, WbemLocator},
        },
    },
    core::{BSTR, PCWSTR},
};

use crate::{
    AtaSmartAttribute, HardwareTelemetrySnapshot, MemoryTelemetry, NvmeHealthValues, Result, StorageDeviceTelemetry,
    StorageReliability, TelemetryError, checked_protocol_window, classify_memory_pressure,
    classify_storage, parse_ata_driver_response, parse_nvme_health_log,
};

static STORAGE_GATE: OnceLock<IsolationGate> = OnceLock::new();
static DIRECT_IOCTL_GATE: OnceLock<IsolationGate> = OnceLock::new();
static MEMORY_GATE: OnceLock<IsolationGate> = OnceLock::new();
fn storage_gate() -> &'static IsolationGate { STORAGE_GATE.get_or_init(IsolationGate::default) }
fn direct_ioctl_gate() -> &'static IsolationGate { DIRECT_IOCTL_GATE.get_or_init(IsolationGate::default) }
fn memory_gate() -> &'static IsolationGate { MEMORY_GATE.get_or_init(IsolationGate::default) }

fn fault_kind(error: &TelemetryError) -> FaultKind {
    match error {
        TelemetryError::Timeout(_) => FaultKind::Timeout,
        TelemetryError::Cancelled(_) => FaultKind::Cancelled,
        TelemetryError::Unavailable(_) => FaultKind::Unavailable,
        TelemetryError::PermissionDenied(_) => FaultKind::PermissionDenied,
        TelemetryError::MalformedResponse(_) => FaultKind::MalformedResponse,
        TelemetryError::Windows(_) => FaultKind::ProviderFailure,
    }
}

fn fault_record(operation: &'static str, error: &TelemetryError) -> CollectorFaultRecord {
    let fault = CollectorFault::new(
        "hardware-telemetry",
        operation,
        fault_kind(error),
        error.to_string(),
    );
    CollectorFaultRecord::from(&fault)
}

fn push_fault_bounded(faults: &mut Vec<CollectorFaultRecord>, fault: CollectorFaultRecord) {
    const MAX_PROVIDER_FAULTS: usize = 32;
    if faults.len() < MAX_PROVIDER_FAULTS { faults.push(fault); }
}

fn checkpoint(control: &CollectorControl, operation: &'static str) -> Result<()> {
    control.checkpoint("hardware-telemetry", operation).map_err(|fault| match fault.kind {
        FaultKind::Timeout => TelemetryError::Timeout(fault.detail),
        FaultKind::Cancelled => TelemetryError::Cancelled(fault.detail),
        _ => TelemetryError::Unavailable(fault.detail),
    })
}

fn init_com() -> Result<ComApartment> {
    initialize_process_com_security().map_err(|e| TelemetryError::Windows(e.to_string()))?;
    ComApartment::mta().map_err(|hr| TelemetryError::Windows(format!("CoInitializeEx failed: 0x{:08X}", hr.0 as u32)))
}

pub fn collect() -> Result<HardwareTelemetrySnapshot> {
    collect_with_cancellation(CancellationToken::new())
}

pub fn collect_with_cancellation(parent: CancellationToken) -> Result<HardwareTelemetrySnapshot> {
    let mut warnings = Vec::new();
    let mut provider_faults = Vec::new();

    let storage = match run_isolated_gated_with_token(
        storage_gate(),
        "hardware-telemetry",
        "storage",
        DEFAULT_COLLECTOR_TIMEOUT,
        parent.child(),
        |control| collect_storage(&control).map_err(|error| {
            CollectorFault::new("hardware-telemetry", "storage", fault_kind(&error), error.to_string())
        }),
    ) {
        Ok((devices, faults)) => {
            provider_faults.extend(faults);
            devices
        }
        Err(error) => {
            warnings.push(format!("Storage telemetry unavailable: {error}"));
            provider_faults.push(CollectorFaultRecord::from(&error));
            Vec::new()
        }
    };

    let memory = match run_isolated_gated_with_token(
        memory_gate(),
        "hardware-telemetry",
        "memory",
        DEFAULT_COLLECTOR_TIMEOUT,
        parent.child(),
        |_control| collect_memory().map_err(|error| {
            CollectorFault::new("hardware-telemetry", "memory", fault_kind(&error), error.to_string())
        }),
    ) {
        Ok(value) => Some(value),
        Err(error) => {
            warnings.push(format!("Memory telemetry unavailable: {error}"));
            provider_faults.push(CollectorFaultRecord::from(&error));
            None
        }
    };

    Ok(HardwareTelemetrySnapshot { storage, memory, provider_faults, warnings })
}

fn collect_memory() -> Result<MemoryTelemetry> {
    let mut status = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    unsafe { GlobalMemoryStatusEx(&mut status) }.map_err(win)?;
    let (pressure_label, pressure_explanation) = classify_memory_pressure(status.dwMemoryLoad);
    Ok(MemoryTelemetry {
        total_physical_bytes: status.ullTotalPhys,
        available_physical_bytes: status.ullAvailPhys,
        memory_load_percent: status.dwMemoryLoad,
        pressure_label,
        pressure_explanation,
    })
}

fn collect_storage(
    control: &CollectorControl,
) -> Result<(Vec<StorageDeviceTelemetry>, Vec<CollectorFaultRecord>)> {
    checkpoint(control, "storage.begin")?;
    let _com = init_com()?;
    let services = connect_storage_wmi()?;
    let mut provider_faults = Vec::new();

    let reliability_result = query_reliability(&services, control);
    let (reliability, reliability_warning) = match reliability_result {
        Ok(value) => (value, None),
        Err(error) => {
            push_fault_bounded(&mut provider_faults, fault_record("wmi.reliability", &error));
            (HashMap::new(), Some(format!("MSFT_StorageReliabilityCounter was unavailable: {error}")))
        }
    };

    let mut devices = query_physical_disks(&services, control)?;
    for device in &mut devices {
        checkpoint(control, "storage.device")?;
        if let Some(note) = &reliability_warning { device.source_notes.push(note.clone()); }
        if let Some(reliability) = reliability.get(&device.device_id) {
            device.reliability = reliability.clone();
            device.source_notes.push("MSFT_StorageReliabilityCounter".into());
        }

        if let Ok(index) = device.device_id.parse::<u32>() {
            if device.bus_type.eq_ignore_ascii_case("NVMe") {
                match query_nvme_health_bounded(index, control.cancellation().child()) {
                    Ok(nvme) => {
                        merge_nvme(&mut device.reliability, nvme);
                        device.source_notes.push("NVMe SMART/Health log via IOCTL_STORAGE_QUERY_PROPERTY".into());
                    }
                    Err(error) => {
                        push_fault_bounded(&mut provider_faults, fault_record("nvme-health-ioctl", &error));
                        device.source_notes.push("Direct NVMe SMART/Health log was not available for this device.".into());
                    }
                }
            } else if device.bus_type.eq_ignore_ascii_case("SATA") || device.bus_type.eq_ignore_ascii_case("ATA") {
                match query_ata_smart_attributes_bounded(index, control.cancellation().child()) {
                    Ok(attributes) if !attributes.is_empty() => {
                        device.ata_smart_attributes = attributes;
                        device.source_notes.push("ATA SMART attribute table via SMART_RCV_DRIVE_DATA; raw values are vendor-defined and are not converted into AetherCore health claims.".into());
                    }
                    Ok(_) => device.source_notes.push("Direct ATA SMART attributes were not available through SMART_RCV_DRIVE_DATA; standardized Windows reliability counters remain authoritative when present.".into()),
                    Err(error) => {
                        push_fault_bounded(&mut provider_faults, fault_record("ata-smart-ioctl", &error));
                        device.source_notes.push("Direct ATA SMART attributes were not available through SMART_RCV_DRIVE_DATA; standardized Windows reliability counters remain authoritative when present.".into());
                    }
                }
            }
        }
        classify_storage(device);
    }
    Ok((devices, provider_faults))
}

fn connect_storage_wmi() -> Result<IWbemServices> {
    unsafe {
        let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).map_err(win)?;
        let services = locator.ConnectServer(&BSTR::from("ROOT\\Microsoft\\Windows\\Storage"), &BSTR::new(), &BSTR::new(), &BSTR::new(), 0, &BSTR::new(), None).map_err(win)?;
        CoSetProxyBlanket(&services, RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE, PCWSTR::null(), RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE, None, EOAC_NONE).map_err(win)?;
        Ok(services)
    }
}

fn query_physical_disks(services: &IWbemServices, control: &CollectorControl) -> Result<Vec<StorageDeviceTelemetry>> {
    let objects = query(services, "SELECT DeviceId,FriendlyName,FirmwareVersion,SerialNumber,BusType,MediaType,Size,HealthStatus,OperationalStatus FROM MSFT_PhysicalDisk", control)?;
    let mut out = Vec::new();
    for o in objects {
        let device_id = prop_string(&o, "DeviceId").unwrap_or_default();
        let mut d = StorageDeviceTelemetry {
            device_id,
            friendly_name: prop_string(&o, "FriendlyName").unwrap_or_else(|| "Physical disk".into()),
            firmware_version: prop_string(&o, "FirmwareVersion").unwrap_or_default(),
            serial_number: prop_string(&o, "SerialNumber").unwrap_or_default(),
            bus_type: bus_name(prop_u16(&o, "BusType")),
            media_type: media_name(prop_u16(&o, "MediaType")),
            size_bytes: prop_u64(&o, "Size").unwrap_or(0),
            windows_health_status: health_name(prop_u16(&o, "HealthStatus")),
            operational_status: Vec::new(),
            ata_smart_attributes: Vec::new(),
            reliability: StorageReliability::default(),
            severity: "Unknown".into(), summary: String::new(), reasons: Vec::new(),
            source_notes: vec!["MSFT_PhysicalDisk".into()],
        };
        if d.device_id.is_empty() { d.source_notes.push("Physical disk DeviceId was not reported.".into()); }
        out.push(d);
    }
    Ok(out)
}

fn query_reliability(services: &IWbemServices, control: &CollectorControl) -> Result<HashMap<String, StorageReliability>> {
    let objects = query(services, "SELECT DeviceId,Temperature,TemperatureMax,Wear,PowerOnHours,ReadErrorsTotal,ReadErrorsUncorrected,WriteErrorsTotal,WriteErrorsUncorrected,ReadLatencyMax,WriteLatencyMax,FlushLatencyMax FROM MSFT_StorageReliabilityCounter", control)?;
    let mut out = HashMap::new();
    for o in objects {
        let Some(device_id) = prop_string(&o, "DeviceId") else { continue; };
        out.insert(device_id, StorageReliability {
            temperature_c: prop_u8(&o, "Temperature").map(i32::from),
            temperature_max_c: prop_u8(&o, "TemperatureMax").map(i32::from),
            wear_percent_used: prop_u8(&o, "Wear").map(u32::from),
            power_on_hours: prop_u16(&o, "PowerOnHours").map(u64::from),
            read_errors_total: prop_u64(&o, "ReadErrorsTotal"),
            read_errors_uncorrected: prop_u64(&o, "ReadErrorsUncorrected"),
            write_errors_total: prop_u64(&o, "WriteErrorsTotal"),
            write_errors_uncorrected: prop_u64(&o, "WriteErrorsUncorrected"),
            read_latency_max_ms: prop_u64(&o, "ReadLatencyMax"),
            write_latency_max_ms: prop_u64(&o, "WriteLatencyMax"),
            flush_latency_max_ms: prop_u64(&o, "FlushLatencyMax"),
            ..Default::default()
        });
    }
    Ok(out)
}

fn query(
    services: &IWbemServices,
    wql: &str,
    control: &CollectorControl,
) -> Result<Vec<windows::Win32::System::Wmi::IWbemClassObject>> {
    unsafe {
        checkpoint(control, "wmi.exec_query")?;
        let flags = WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY;
        let e = services.ExecQuery(&BSTR::from("WQL"), &BSTR::from(wql), flags, None).map_err(win)?;
        let mut out = Vec::new();
        loop {
            checkpoint(control, "wmi.next")?;
            let mut values = [None];
            let mut returned = 0u32;
            let timeout_ms = control.remaining_ms_capped(WMI_NEXT_SLICE);
            let timeout_ms = i32::try_from(timeout_ms).unwrap_or(i32::MAX);
            let status = e.Next(timeout_ms, &mut values, &mut returned);

            // IEnumWbemClassObject::Next returns HRESULT directly in windows-rs. WBEM_S_TIMEDOUT
            // is a successful status and explicitly means the finite slice expired before the
            // requested object count was satisfied; it must not be mistaken for end-of-stream.
            if status.0 == WBEM_S_TIMEDOUT.0 {
                if let Some(v) = take_wmi_object(&mut values, returned)? { out.push(v); }
                if out.len() > 256 {
                    return Err(TelemetryError::MalformedResponse("WMI provider returned more than the bounded 256-object storage inventory".into()));
                }
                continue;
            }
            if status.is_err() {
                let detail = format!("IEnumWbemClassObject::Next failed with HRESULT 0x{:08X}", status.0 as u32);
                return Err(if status.0 == E_ACCESSDENIED.0 || status.0 == WBEM_E_ACCESS_DENIED.0 {
                    TelemetryError::PermissionDenied(detail)
                } else {
                    TelemetryError::Windows(detail)
                });
            }

            if status.0 == WBEM_S_FALSE.0 {
                if returned != 0 || values[0].is_some() {
                    return Err(TelemetryError::MalformedResponse(
                        "WMI enumerator returned terminal WBEM_S_FALSE with a non-empty result".into(),
                    ));
                }
                break;
            }
            if let Some(v) = take_wmi_object(&mut values, returned)? { out.push(v); }
            if out.len() > 256 {
                return Err(TelemetryError::MalformedResponse("WMI provider returned more than the bounded 256-object storage inventory".into()));
            }
            if returned == 0 {
                return Err(TelemetryError::MalformedResponse(
                    "WMI enumerator returned zero objects without WBEM_S_TIMEDOUT or WBEM_S_FALSE".into(),
                ));
            }
        }
        Ok(out)
    }
}

fn take_wmi_object(
    values: &mut [Option<windows::Win32::System::Wmi::IWbemClassObject>; 1],
    returned: u32,
) -> Result<Option<windows::Win32::System::Wmi::IWbemClassObject>> {
    if returned > 1 {
        return Err(TelemetryError::MalformedResponse(
            "WMI enumerator reported more objects than the supplied output slice".into(),
        ));
    }
    match (returned, values[0].take()) {
        (0, None) => Ok(None),
        (1, Some(value)) => Ok(Some(value)),
        (0, Some(_)) => Err(TelemetryError::MalformedResponse(
            "WMI enumerator populated an object while reporting zero returned objects".into(),
        )),
        (1, None) => Err(TelemetryError::MalformedResponse(
            "WMI enumerator reported one returned object but supplied no object".into(),
        )),
        _ => Err(TelemetryError::MalformedResponse("WMI enumerator returned an invalid object count".into())),
    }
}

fn get_variant(o: &windows::Win32::System::Wmi::IWbemClassObject, name: &str) -> Option<VARIANT> {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut value = VARIANT::default();
    unsafe { o.Get(PCWSTR(wide.as_ptr()), 0, &mut value, None, None) }.ok()?;
    Some(value)
}
fn prop_string(o: &windows::Win32::System::Wmi::IWbemClassObject, n: &str) -> Option<String> { BSTR::try_from(&get_variant(o,n)?).ok().map(|v| v.to_string()) }
fn prop_u8(o: &windows::Win32::System::Wmi::IWbemClassObject, n: &str) -> Option<u8> { u16::try_from(&get_variant(o,n)?).ok().and_then(|v|u8::try_from(v).ok()) }
fn prop_u16(o: &windows::Win32::System::Wmi::IWbemClassObject, n: &str) -> Option<u16> { u16::try_from(&get_variant(o,n)?).ok().or_else(|| u32::try_from(&get_variant(o,n)?).ok().and_then(|v|u16::try_from(v).ok())) }
fn prop_u64(o: &windows::Win32::System::Wmi::IWbemClassObject, n: &str) -> Option<u64> { u64::try_from(&get_variant(o,n)?).ok().or_else(|| u32::try_from(&get_variant(o,n)?).ok().map(u64::from)) }

fn bus_name(v: Option<u16>) -> String { match v { Some(3)=>"ATA", Some(11)=>"SATA", Some(17)=>"NVMe", Some(7)=>"USB", Some(8)=>"RAID", Some(9)=>"iSCSI", Some(10)=>"SAS", Some(n)=>return format!("BusType {n}"), None=>"Unknown" }.into() }
fn media_name(v: Option<u16>) -> String { match v { Some(3)=>"HDD", Some(4)=>"SSD", Some(5)=>"SCM", Some(n)=>return format!("MediaType {n}"), None=>"Unknown" }.into() }
fn health_name(v: Option<u16>) -> String { match v { Some(0)=>"Healthy", Some(1)=>"Warning", Some(2)=>"Unhealthy", Some(5)=>"Unknown", Some(n)=>return format!("HealthStatus {n}"), None=>"Unknown" }.into() }

fn isolated_fault_to_telemetry(fault: CollectorFault) -> TelemetryError {
    match fault.kind {
        FaultKind::Timeout => TelemetryError::Timeout(fault.detail),
        FaultKind::Cancelled => TelemetryError::Cancelled(fault.detail),
        FaultKind::MalformedResponse => TelemetryError::MalformedResponse(fault.detail),
        FaultKind::Unavailable => TelemetryError::Unavailable(fault.detail),
        FaultKind::PermissionDenied => TelemetryError::PermissionDenied(fault.detail),
        FaultKind::ProviderFailure | FaultKind::Io | FaultKind::Internal => TelemetryError::Windows(fault.detail),
    }
}

fn query_ata_smart_attributes_bounded(index: u32, token: CancellationToken) -> Result<Vec<AtaSmartAttribute>> {
    run_isolated_gated_with_token(
        direct_ioctl_gate(),
        "hardware-telemetry",
        "ata-smart-ioctl",
        STORAGE_IOCTL_TIMEOUT,
        token,
        move |_control| query_ata_smart_attributes(index).map_err(|error| {
            CollectorFault::new("hardware-telemetry", "ata-smart-ioctl", fault_kind(&error), error.to_string())
        }),
    ).map_err(isolated_fault_to_telemetry)
}

fn query_nvme_health_bounded(index: u32, token: CancellationToken) -> Result<NvmeHealthValues> {
    run_isolated_gated_with_token(
        direct_ioctl_gate(),
        "hardware-telemetry",
        "nvme-health-ioctl",
        STORAGE_IOCTL_TIMEOUT,
        token,
        move |_control| query_nvme_health(index).map_err(|error| {
            CollectorFault::new("hardware-telemetry", "nvme-health-ioctl", fault_kind(&error), error.to_string())
        }),
    ).map_err(isolated_fault_to_telemetry)
}

fn query_ata_smart_attributes(index: u32) -> Result<Vec<AtaSmartAttribute>> {
    let path = format!(r"\\.\PhysicalDrive{index}");
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    const GENERIC_READ_ACCESS: u32 = 0x8000_0000;
    let handle = unsafe { CreateFileW(PCWSTR(wide.as_ptr()), GENERIC_READ_ACCESS, FILE_SHARE_READ | FILE_SHARE_WRITE, None, OPEN_EXISTING, FILE_FLAGS_AND_ATTRIBUTES(FILE_ATTRIBUTE_NORMAL.0), None) }.map_err(win)?;
    let _handle_owner = OwnedHandle::new(handle);

    // SMART_RCV_DRIVE_DATA is read-only. The target disk is selected by the handle; bDriveNumber is opaque to callers.
    let input = SENDCMDINPARAMS {
        cBufferSize: 512,
        irDriveRegs: IDEREGS {
            bFeaturesReg: READ_ATTRIBUTES as u8,
            bSectorCountReg: 1,
            bSectorNumberReg: 1,
            bCylLowReg: SMART_CYL_LOW as u8,
            bCylHighReg: SMART_CYL_HI as u8,
            bDriveHeadReg: 0xA0,
            bCommandReg: SMART_CMD as u8,
            bReserved: 0,
        },
        bDriveNumber: u8::try_from(index)
            .map_err(|_| TelemetryError::MalformedResponse("physical-drive index did not fit ATA SMART bDriveNumber".into()))?,
        bReserved: [0; 3],
        dwReserved: [0; 4],
        bBuffer: [0; 1],
    };
    let input_len = size_of::<SENDCMDINPARAMS>() - 1;
    let data_offset = offset_of!(SENDCMDOUTPARAMS, bBuffer);
    let output_len = data_offset + 512;
    let mut output = vec![0u8; output_len];
    let mut returned = 0u32;
    unsafe {
        DeviceIoControl(
            handle,
            SMART_RCV_DRIVE_DATA,
            Some((&input as *const SENDCMDINPARAMS).cast()),
            input_len as u32,
            Some(output.as_mut_ptr().cast()),
            output_len as u32,
            Some(&mut returned),
            None,
        )
    }.map_err(win)?;
    let returned = usize::try_from(returned)
        .map_err(|_| TelemetryError::MalformedResponse("ATA SMART byte count did not fit usize".into()))?;
    parse_ata_driver_response(&output, returned, data_offset)
}

fn query_nvme_health(index: u32) -> Result<NvmeHealthValues> {
    let path = format!(r"\\.\PhysicalDrive{index}");
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    const GENERIC_READ_ACCESS:u32=0x8000_0000;
    let handle = unsafe { CreateFileW(PCWSTR(wide.as_ptr()), GENERIC_READ_ACCESS, FILE_SHARE_READ | FILE_SHARE_WRITE, None, OPEN_EXISTING, FILE_FLAGS_AND_ATTRIBUTES(FILE_ATTRIBUTE_NORMAL.0), None) }.map_err(win)?;
    let _handle_owner = OwnedHandle::new(handle);

    // STORAGE_PROPERTY_QUERY ends in a one-byte flexible array. Microsoft requires the protocol
    // query to begin at AdditionalParameters rather than after sizeof(STORAGE_PROPERTY_QUERY).
    let query_prefix=offset_of!(STORAGE_PROPERTY_QUERY,AdditionalParameters);
    let input_len=query_prefix+size_of::<STORAGE_PROTOCOL_SPECIFIC_DATA>();
    let mut input_words=vec![0u64;(input_len+7)/8];
    let input_ptr=input_words.as_mut_ptr() as *mut u8;
    unsafe {
        let q=&mut *(input_ptr as *mut STORAGE_PROPERTY_QUERY);
        q.PropertyId=StorageDeviceProtocolSpecificProperty;q.QueryType=PropertyStandardQuery;
        let p=&mut *(input_ptr.add(query_prefix) as *mut STORAGE_PROTOCOL_SPECIFIC_DATA);
        p.ProtocolType=ProtocolTypeNvme;
        p.DataType=2; // NVMeDataTypeLogPage
        p.ProtocolDataRequestValue=2; // SMART / Health Information log page (02h)
        p.ProtocolDataOffset=size_of::<STORAGE_PROTOCOL_SPECIFIC_DATA>() as u32;
        p.ProtocolDataLength=512;
    }

    const NVME_HEALTH_LOG_BYTES: usize = 512;
    let out_len=size_of::<STORAGE_PROTOCOL_DATA_DESCRIPTOR>()+NVME_HEALTH_LOG_BYTES+64;
    let mut output_words=vec![0u64;(out_len+7)/8];let mut returned=0u32;
    let output_ptr=output_words.as_mut_ptr() as *mut u8;
    unsafe { DeviceIoControl(handle,IOCTL_STORAGE_QUERY_PROPERTY,Some(input_ptr as *const _),input_len as u32,Some(output_ptr as *mut _),out_len as u32,Some(&mut returned),None) }.map_err(win)?;
    let returned = usize::try_from(returned).map_err(|_| TelemetryError::MalformedResponse("NVMe IOCTL byte count did not fit usize".into()))?;
    if returned > out_len { return Err(TelemetryError::MalformedResponse("NVMe driver reported more bytes than the supplied output buffer".into())); }
    if returned < size_of::<STORAGE_PROTOCOL_DATA_DESCRIPTOR>() {return Err(TelemetryError::MalformedResponse("NVMe protocol descriptor was not returned".into()));}
    let descriptor=unsafe{std::ptr::read_unaligned(output_ptr as *const STORAGE_PROTOCOL_DATA_DESCRIPTOR)};
    let descriptor_min=size_of::<STORAGE_PROTOCOL_DATA_DESCRIPTOR>() as u32;
    if descriptor.Version < descriptor_min || descriptor.Size < descriptor_min || usize::try_from(descriptor.Size).ok().is_none_or(|size| size > returned) {
        return Err(TelemetryError::MalformedResponse("NVMe protocol descriptor version/size was invalid".into()));
    }
    let protocol=&descriptor.ProtocolSpecificData;
    if protocol.ProtocolType != ProtocolTypeNvme || protocol.DataType != 2 {
        return Err(TelemetryError::MalformedResponse("NVMe driver returned an unexpected protocol/data type".into()));
    }
    let protocol_offset=offset_of!(STORAGE_PROTOCOL_DATA_DESCRIPTOR, ProtocolSpecificData);
    let window=checked_protocol_window(returned,protocol_offset,protocol.ProtocolDataOffset,protocol.ProtocolDataLength,size_of::<STORAGE_PROTOCOL_SPECIFIC_DATA>(),NVME_HEALTH_LOG_BYTES)?;
    let output_slice=unsafe{std::slice::from_raw_parts(output_ptr,returned)};
    parse_nvme_health_log(&output_slice[window])
}
fn merge_nvme(r:&mut StorageReliability,n:NvmeHealthValues){r.nvme_critical_warning=Some(n.critical);r.nvme_available_spare_percent=Some(n.spare);r.nvme_percentage_used=Some(n.used);r.nvme_unsafe_shutdowns=Some(n.unsafe_shutdowns);r.nvme_media_errors=Some(n.media_errors);r.nvme_error_log_entries=Some(n.error_entries);if r.temperature_c.is_none(){r.temperature_c=n.temperature_c;}if r.wear_percent_used.is_none(){r.wear_percent_used=Some(n.used as u32);}}

fn win(e: windows::core::Error) -> TelemetryError {
    if e.code() == E_ACCESSDENIED || e.code().0 == WBEM_E_ACCESS_DENIED.0 { TelemetryError::PermissionDenied(e.to_string()) }
    else { TelemetryError::Windows(e.to_string()) }
}
