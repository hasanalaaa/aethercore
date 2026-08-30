use std::mem::size_of;

use windows::{
    Win32::{
        Devices::DeviceAndDriverInstallation::{
            CM_Get_DevNode_Status, CR_SUCCESS, DICS_FLAG_GLOBAL, DIGCF_ALLCLASSES, DIGCF_PRESENT,
            DN_HAS_PROBLEM,
            DIREG_DRV, HDEVINFO, SP_DEVINFO_DATA, SPDRP_CLASS, SPDRP_CLASSGUID,
            SPDRP_COMPATIBLEIDS, SPDRP_DEVICEDESC, SPDRP_ENUMERATOR_NAME, SPDRP_FRIENDLYNAME,
            SPDRP_HARDWAREID, SPDRP_LOCATION_INFORMATION, SPDRP_MFG, SetupDiDestroyDeviceInfoList,
            SetupDiEnumDeviceInfo, SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW,
            SetupDiGetDeviceRegistryPropertyW, SetupDiOpenDevRegKey,
        },
        Foundation::ERROR_NO_MORE_ITEMS,
        System::Registry::{HKEY, KEY_READ, REG_MULTI_SZ, REG_SZ, RegCloseKey, RegQueryValueExW},
    },
    core::PCWSTR,
};

use crate::{
    DeviceRecord, DeviceStatus, DeviceVerification, InstalledDriver, PnpError, Result, is_missing_driver_problem,
    parse_multi_sz_utf16,
};

struct DeviceInfoSet(HDEVINFO);

impl Drop for DeviceInfoSet {
    fn drop(&mut self) {
        unsafe {
            let _ = SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

struct RegistryKey(HKEY);

impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

pub fn scan_present_devices() -> Result<Vec<DeviceRecord>> {
    let set = unsafe {
        SetupDiGetClassDevsW(
            None,
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_ALLCLASSES,
        )
        .map(DeviceInfoSet)
        .map_err(win_err)?
    };

    let mut devices = Vec::new();
    let mut index = 0u32;
    loop {
        let mut info = SP_DEVINFO_DATA {
            cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };

        match unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut info) } {
            Ok(()) => {}
            Err(error) if error.code() == ERROR_NO_MORE_ITEMS.to_hresult() => break,
            Err(error) => {
                return Err(PnpError::Windows(format!(
                    "SetupDiEnumDeviceInfo failed at index {index}: {error}"
                )));
            }
        }
        index += 1;

        let instance_id = read_instance_id(set.0, &info).ok_or_else(|| {
            PnpError::Windows(format!(
                "unable to read a usable device instance ID for devinst {}",
                info.DevInst
            ))
        })?;
        let enumerator = read_string_property(set.0, &info, SPDRP_ENUMERATOR_NAME).unwrap_or_default();

        // SWD is Windows' software-device enumerator. Phase 2 intentionally inventories
        // present hardware/PnP devices rather than virtual software-only endpoints.
        if enumerator.eq_ignore_ascii_case("SWD") || instance_id.to_ascii_uppercase().starts_with("SWD\\") {
            continue;
        }

        let friendly = read_string_property(set.0, &info, SPDRP_FRIENDLYNAME).unwrap_or_default();
        let description = read_string_property(set.0, &info, SPDRP_DEVICEDESC).unwrap_or_default();
        let display_name = if friendly.trim().is_empty() {
            if description.trim().is_empty() { instance_id.clone() } else { description.clone() }
        } else {
            friendly
        };

        let (raw_status, reported_problem_code) = read_devnode_status(info.DevInst)?;
        // Per Configuration Manager, the problem number is meaningful only when
        // DN_HAS_PROBLEM is present in the devnode status flags. Do not infer a
        // problem merely because a stale/non-zero output value was observed.
        let has_problem = (raw_status & DN_HAS_PROBLEM.0) != 0;
        let problem_code = if has_problem { reported_problem_code } else { 0 };
        let driver = read_driver_metadata(set.0, &info);

        devices.push(DeviceRecord {
            instance_id,
            display_name,
            description,
            class_name: read_string_property(set.0, &info, SPDRP_CLASS).unwrap_or_default(),
            class_guid: read_string_property(set.0, &info, SPDRP_CLASSGUID).unwrap_or_default(),
            manufacturer: read_string_property(set.0, &info, SPDRP_MFG).unwrap_or_default(),
            enumerator,
            location: read_string_property(set.0, &info, SPDRP_LOCATION_INFORMATION).unwrap_or_default(),
            hardware_ids: read_multi_sz_property(set.0, &info, SPDRP_HARDWAREID).unwrap_or_default(),
            compatible_ids: read_multi_sz_property(set.0, &info, SPDRP_COMPATIBLEIDS).unwrap_or_default(),
            status: DeviceStatus {
                raw_status,
                problem_code,
                has_problem,
                missing_driver: has_problem && is_missing_driver_problem(problem_code),
            },
            driver,
        });
    }

    devices.sort_by(|a, b| {
        a.class_name
            .cmp(&b.class_name)
            .then_with(|| a.display_name.cmp(&b.display_name))
            .then_with(|| a.instance_id.cmp(&b.instance_id))
    });
    Ok(devices)
}

fn read_devnode_status(devinst: u32) -> Result<(u32, u32)> {
    let mut status = Default::default();
    let mut problem = Default::default();
    let result = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, devinst, 0) };
    if result == CR_SUCCESS {
        Ok((status.0, problem.0))
    } else {
        Err(PnpError::Windows(format!(
            "CM_Get_DevNode_Status failed for devinst {devinst}: CONFIGRET {}",
            result.0
        )))
    }
}

fn read_instance_id(set: HDEVINFO, info: &SP_DEVINFO_DATA) -> Option<String> {
    let mut required = 0u32;
    unsafe {
        let _ = SetupDiGetDeviceInstanceIdW(set, info, None, Some(&mut required));
    }
    if required == 0 || required > 32 * 1024 {
        return None;
    }
    let mut buffer = vec![0u16; required as usize];
    unsafe {
        SetupDiGetDeviceInstanceIdW(set, info, Some(&mut buffer), Some(&mut required)).ok()?;
    }
    let end = buffer.iter().position(|v| *v == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..end]))
}

fn read_string_property(
    set: HDEVINFO,
    info: &SP_DEVINFO_DATA,
    property: windows::Win32::Devices::DeviceAndDriverInstallation::SETUP_DI_REGISTRY_PROPERTY,
) -> Option<String> {
    let (ty, bytes) = read_property_bytes(set, info, property)?;
    if ty != REG_SZ.0 {
        return None;
    }
    let units = bytes_to_u16(&bytes);
    let end = units.iter().position(|v| *v == 0).unwrap_or(units.len());
    Some(String::from_utf16_lossy(&units[..end]).trim().to_string())
}

fn read_multi_sz_property(
    set: HDEVINFO,
    info: &SP_DEVINFO_DATA,
    property: windows::Win32::Devices::DeviceAndDriverInstallation::SETUP_DI_REGISTRY_PROPERTY,
) -> Option<Vec<String>> {
    let (ty, bytes) = read_property_bytes(set, info, property)?;
    if ty != REG_MULTI_SZ.0 {
        return None;
    }
    Some(parse_multi_sz_utf16(&bytes_to_u16(&bytes)))
}

fn read_property_bytes(
    set: HDEVINFO,
    info: &SP_DEVINFO_DATA,
    property: windows::Win32::Devices::DeviceAndDriverInstallation::SETUP_DI_REGISTRY_PROPERTY,
) -> Option<(u32, Vec<u8>)> {
    let mut required = 0u32;
    let mut ty = 0u32;
    unsafe {
        let _ = SetupDiGetDeviceRegistryPropertyW(
            set,
            info,
            property,
            Some(&mut ty),
            None,
            Some(&mut required),
        );
    }
    if required == 0 || required > 256 * 1024 {
        return None;
    }
    let mut buffer = vec![0u8; required as usize];
    unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set,
            info,
            property,
            Some(&mut ty),
            Some(&mut buffer),
            Some(&mut required),
        )
        .ok()?;
    }
    buffer.truncate(required as usize);
    Some((ty, buffer))
}

fn read_driver_metadata(set: HDEVINFO, info: &SP_DEVINFO_DATA) -> Option<InstalledDriver> {
    let key = unsafe {
        SetupDiOpenDevRegKey(
            set,
            info,
            DICS_FLAG_GLOBAL.0,
            0,
            DIREG_DRV,
            KEY_READ.0,
        )
        .ok()
        .map(RegistryKey)?
    };

    let provider = read_reg_string(key.0, "ProviderName").unwrap_or_default();
    let version = read_reg_string(key.0, "DriverVersion").unwrap_or_default();
    let inf_path = read_reg_string(key.0, "InfPath").unwrap_or_default();
    let date = read_reg_string(key.0, "DriverDate").unwrap_or_default();

    if provider.is_empty() && version.is_empty() && inf_path.is_empty() && date.is_empty() {
        None
    } else {
        Some(InstalledDriver {
            provider,
            version,
            inf_path,
            date,
        })
    }
}

fn read_reg_string(key: HKEY, name: &str) -> Option<String> {
    let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut ty = Default::default();
    let mut size = 0u32;
    let first = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&mut ty),
            None,
            Some(&mut size),
        )
    };
    if first.0 != 0 || size == 0 || size > 64 * 1024 || ty != REG_SZ {
        return None;
    }

    let mut bytes = vec![0u8; size as usize];
    let second = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&mut ty),
            Some(bytes.as_mut_ptr()),
            Some(&mut size),
        )
    };
    if second.0 != 0 {
        return None;
    }
    bytes.truncate(size as usize);
    let units = bytes_to_u16(&bytes);
    let end = units.iter().position(|v| *v == 0).unwrap_or(units.len());
    Some(String::from_utf16_lossy(&units[..end]).trim().to_string())
}

fn bytes_to_u16(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect()
}

fn win_err(error: windows::core::Error) -> PnpError {
    PnpError::Windows(error.to_string())
}


pub fn verify_device_instances(instance_ids: &[String]) -> Result<Vec<DeviceVerification>> {
    use std::collections::HashMap;
    let inventory = scan_present_devices()?;
    let by_id: HashMap<String, DeviceRecord> = inventory
        .into_iter()
        .map(|device| (crate::normalize_pnp_id(&device.instance_id), device))
        .collect();

    let mut out = Vec::with_capacity(instance_ids.len());
    for requested in instance_ids {
        let key = crate::normalize_pnp_id(requested);
        if let Some(device) = by_id.get(&key) {
            out.push(DeviceVerification {
                instance_id: device.instance_id.clone(),
                present: true,
                class_name: device.class_name.clone(),
                hardware_ids: device.hardware_ids.clone(),
                compatible_ids: device.compatible_ids.clone(),
                status: device.status.clone(),
                driver: device.driver.clone(),
            });
        } else {
            out.push(DeviceVerification {
                instance_id: requested.clone(),
                present: false,
                class_name: String::new(),
                hardware_ids: Vec::new(),
                compatible_ids: Vec::new(),
                status: DeviceStatus::default(),
                driver: None,
            });
        }
    }
    Ok(out)
}
