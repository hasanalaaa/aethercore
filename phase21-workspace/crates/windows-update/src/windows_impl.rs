use aethercore_windows_foundation::ComApartment;
use windows::{
    Win32::{
        Foundation::{DECIMAL, VARIANT_BOOL, VARIANT_FALSE, VARIANT_TRUE},
        System::{
            Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            UpdateAgent::{
                IUpdateSession, IWindowsDriverUpdate, IWindowsDriverUpdate4,
                IWindowsDriverUpdateEntry, UpdateSession, orcSucceeded, orcSucceededWithErrors,
            },
        },
    },
    core::{BSTR, Interface},
};

use crate::{
    DiscoveryResult, DriverOffer, Result, SearchScope, UpdateError, UpdateHealthProbe,
    VersionSource, extract_version_from_update_title, ole_automation_date_to_iso,
};

#[derive(Clone)]
struct CommonOfferMetadata {
    update_id: String,
    revision: i32,
    title: String,
    min_download_bytes: u64,
    max_download_bytes: u64,
    target_version: String,
    target_version_source: VersionSource,
    support_url: String,
}

pub fn probe_update_health() -> UpdateHealthProbe {
    let guard = match ComApartment::mta() {
        Ok(value) => value,
        Err(hr) => {
            return UpdateHealthProbe {
                result_code: "UpdateUnknown".into(),
                hresult: hr.0,
                pending_update_count: 0,
                detail: format!("CoInitializeEx failed: 0x{:08X}", hr.0 as u32),
            };
        }
    };
    let _guard = guard;
    let session: IUpdateSession =
        match unsafe { CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER) } {
            Ok(value) => value,
            Err(error) => return update_probe_error(error),
        };
    let app_id = BSTR::from("AetherCore Windows Health");
    if let Err(error) = unsafe { session.SetClientApplicationID(&app_id) } {
        return update_probe_error(error);
    }
    let searcher = match unsafe { session.CreateUpdateSearcher() } {
        Ok(value) => value,
        Err(error) => return update_probe_error(error),
    };
    if let Err(error) = unsafe { searcher.SetOnline(VARIANT_BOOL(-1)) } {
        return update_probe_error(error);
    }
    let criteria = BSTR::from("IsInstalled=0 and IsHidden=0");
    let result = match unsafe { searcher.Search(&criteria) } {
        Ok(value) => value,
        Err(error) => return update_probe_error(error),
    };
    let result_code = match unsafe { result.ResultCode() } {
        Ok(value) => value,
        Err(error) => return update_probe_error(error),
    };
    let updates = match unsafe { result.Updates() } {
        Ok(value) => value,
        Err(error) => return update_probe_error(error),
    };
    // DBT-P46-B15: a failed Count() after a SUCCESSFUL search used to report
    // "0 pending updates" as if measured. pending_update_count is not on the
    // wire (verified: no .proto, protocol.rs or aetherctl reference), so per
    // Hasan's instruction this carries the distinction in the existing detail
    // string rather than adding a field nothing consumes.
    let counted = unsafe { updates.Count() }
        .ok()
        .map(|value| value.max(0) as u32);
    let count = counted.unwrap_or(0);
    if result_code == orcSucceeded {
        match counted {
            Some(count) => UpdateHealthProbe { result_code:"UpdateHealthy".into(), hresult:0, pending_update_count:count, detail:format!("Windows Update Agent discovery completed successfully; pending applicable updates: {count}.") },
            None => UpdateHealthProbe { result_code:"UpdateHealthy".into(), hresult:0, pending_update_count:0, detail:"Windows Update Agent discovery completed successfully, but the pending-update count could not be read; the reported 0 is not a measurement.".into() },
        }
    } else if result_code == orcSucceededWithErrors {
        UpdateHealthProbe {
            result_code: "UpdateFailure".into(),
            hresult: 0,
            pending_update_count: count,
            detail:
                "Windows Update Agent discovery completed with errors; results may be incomplete."
                    .into(),
        }
    } else {
        UpdateHealthProbe {
            result_code: "UpdateFailure".into(),
            hresult: 0,
            pending_update_count: count,
            detail: format!("Windows Update Agent discovery returned {result_code:?}."),
        }
    }
}

fn update_probe_error(error: windows::core::Error) -> UpdateHealthProbe {
    let hr = error.code().0;
    let code = hr as u32;
    // Only WU_E_NO_CONNECTION is classified as offline here. Proxy/DNS-specific codes remain
    // update failures until corroborating evidence exists; this avoids inventing a root cause.
    let result = if code == 0x8024_001F {
        "UpdateOffline"
    } else {
        "UpdateFailure"
    };
    UpdateHealthProbe {
        result_code: result.into(),
        hresult: hr,
        pending_update_count: 0,
        detail: format!("Windows Update Agent HRESULT 0x{code:08X}: {error}"),
    }
}

pub fn discover_driver_offers(scope: SearchScope) -> Result<DiscoveryResult> {
    let _guard = ComApartment::mta()
        .map_err(|hr| UpdateError::Wua(format!("CoInitializeEx failed: 0x{:08X}", hr.0 as u32)))?;

    let session: IUpdateSession =
        unsafe { CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER).map_err(wua_err)? };
    let app_id = BSTR::from("AetherCore Driver Discovery");
    unsafe { session.SetClientApplicationID(&app_id).map_err(wua_err)? };
    let searcher = unsafe { session.CreateUpdateSearcher().map_err(wua_err)? };

    // Always set explicitly, never left to WUA's default. VARIANT_FALSE means the search uses
    // local data only ([MS-UAMG] 3.38.4.13). An online search keeps the machine's configured
    // update source (Microsoft Update/Windows Update or an enterprise WSUS policy selected by WUA).
    let online = match scope {
        SearchScope::Online => VARIANT_TRUE,
        SearchScope::LocalCacheOnly => VARIANT_FALSE,
    };
    unsafe {
        searcher.SetOnline(online).map_err(wua_err)?;
    }

    // WUA evaluates applicability. AetherCore never ranks packages by version number.
    let criteria = BSTR::from("IsInstalled=0 and Type='Driver' and IsHidden=0");
    let result = unsafe { searcher.Search(&criteria).map_err(wua_err)? };
    let result_code = unsafe { result.ResultCode().map_err(wua_err)? };
    let mut warnings = Vec::new();
    if result_code == orcSucceededWithErrors {
        warnings.push(
            "Windows Update search completed with errors; applicability results may be incomplete."
                .to_string(),
        );
    } else if result_code != orcSucceeded {
        return Err(UpdateError::Wua(format!(
            "Windows Update search did not complete successfully: {result_code:?}"
        )));
    }

    let collection = unsafe { result.Updates().map_err(wua_err)? };
    let count = unsafe { collection.Count().map_err(wua_err)? };

    let mut offers = Vec::with_capacity(count.max(0) as usize);

    for index in 0..count {
        let update = match unsafe { collection.get_Item(index) } {
            Ok(value) => value,
            Err(error) => {
                warnings.push(format!("WUA item {index} could not be read: {error}"));
                continue;
            }
        };
        let driver: IWindowsDriverUpdate = match update.cast() {
            Ok(value) => value,
            Err(_) => {
                warnings.push(format!(
                    "WUA item {index} reported Type=Driver but did not expose IWindowsDriverUpdate"
                ));
                continue;
            }
        };

        let common = match read_common_metadata(&driver) {
            Ok(value) => value,
            Err(error) => {
                warnings.push(format!(
                    "WUA driver item {index} metadata was rejected: {error}"
                ));
                continue;
            }
        };

        // IWindowsDriverUpdate4 is the authoritative way to enumerate every device-specific
        // applicability entry carried by one WUA update. Older agents may not expose it, so we
        // retain the base IWindowsDriverUpdate fields as a compatibility fallback.
        let mut expanded = false;
        if let Ok(driver4) = driver.cast::<IWindowsDriverUpdate4>() {
            match unsafe { driver4.WindowsDriverUpdateEntries() } {
                Ok(entries) => match unsafe { entries.Count() } {
                    Ok(entry_count) if entry_count > 0 => {
                        let mut successful_entries = 0usize;
                        for entry_index in 0..entry_count {
                            let entry = match unsafe { entries.get_Item(entry_index) } {
                                Ok(value) => value,
                                Err(error) => {
                                    warnings.push(format!(
                                        "WUA item {index} entry {entry_index} could not be read: {error}"
                                    ));
                                    continue;
                                }
                            };
                            match offer_from_entry(&common, &entry) {
                                Ok(offer) => {
                                    offers.push(offer);
                                    successful_entries += 1;
                                }
                                Err(error) => warnings.push(format!(
                                    "WUA item {index} entry {entry_index} metadata was rejected: {error}"
                                )),
                            }
                        }
                        // If an agent exposes IWindowsDriverUpdate4 but every entry is unreadable,
                        // retain the base IWindowsDriverUpdate applicability record instead of
                        // accidentally dropping the entire driver offer.
                        expanded = successful_entries > 0;
                    }
                    Ok(_) => {}
                    Err(error) => warnings.push(format!(
                        "WUA item {index} driver-entry count could not be read: {error}"
                    )),
                },
                Err(error) => warnings.push(format!(
                    "WUA item {index} exposed IWindowsDriverUpdate4 but its entry collection could not be read: {error}"
                )),
            }
        }

        if !expanded {
            match offer_from_base(&common, &driver) {
                Ok(offer) => offers.push(offer),
                Err(error) => warnings.push(format!(
                    "WUA driver item {index} applicability metadata was rejected: {error}"
                )),
            }
        }
    }

    offers.sort_by(|a, b| {
        a.driver_class
            .cmp(&b.driver_class)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.update_id.cmp(&b.update_id))
            .then_with(|| a.hardware_id.cmp(&b.hardware_id))
    });
    offers.dedup_by(|a, b| {
        a.update_id == b.update_id
            && a.revision == b.revision
            && a.hardware_id.eq_ignore_ascii_case(&b.hardware_id)
    });

    Ok(DiscoveryResult { offers, warnings })
}

fn read_common_metadata(driver: &IWindowsDriverUpdate) -> Result<CommonOfferMetadata> {
    let identity = unsafe { driver.Identity().map_err(wua_err)? };
    let update_id = bounded(
        unsafe { identity.UpdateID().map_err(wua_err)?.to_string() },
        128,
    );
    let revision = unsafe { identity.RevisionNumber().map_err(wua_err)? };
    let title = bounded(
        unsafe { driver.Title().map_err(wua_err)?.to_string() },
        1024,
    );
    let target_version = extract_version_from_update_title(&title).unwrap_or_default();
    let target_version_source = if target_version.is_empty() {
        VersionSource::Unavailable
    } else {
        VersionSource::TitleHeuristic
    };

    let min_download_bytes =
        unsafe { decimal_to_u64(&driver.MinDownloadSize().map_err(wua_err)?)? };
    let max_download_bytes =
        unsafe { decimal_to_u64(&driver.MaxDownloadSize().map_err(wua_err)?)? };
    if max_download_bytes < min_download_bytes {
        return Err(UpdateError::InvalidSize);
    }

    Ok(CommonOfferMetadata {
        update_id,
        revision,
        title,
        min_download_bytes,
        max_download_bytes,
        target_version,
        target_version_source,
        // Informational metadata only. The desktop never opens this arbitrary value; GPU actions
        // use a fixed allowlist of official vendor URLs.
        support_url: bounded(
            unsafe {
                driver
                    .SupportUrl()
                    .map(|value| value.to_string())
                    .unwrap_or_default()
            },
            2048,
        ),
    })
}

fn offer_from_entry(
    common: &CommonOfferMetadata,
    entry: &IWindowsDriverUpdateEntry,
) -> Result<DriverOffer> {
    Ok(DriverOffer {
        update_id: common.update_id.clone(),
        revision: common.revision,
        title: common.title.clone(),
        hardware_id: bounded(
            unsafe { entry.DriverHardwareID().map_err(wua_err)?.to_string() },
            4096,
        ),
        driver_class: bounded(
            unsafe { entry.DriverClass().map_err(wua_err)?.to_string() },
            512,
        ),
        manufacturer: bounded(
            unsafe { entry.DriverManufacturer().map_err(wua_err)?.to_string() },
            512,
        ),
        model: bounded(
            unsafe { entry.DriverModel().map_err(wua_err)?.to_string() },
            512,
        ),
        provider: bounded(
            unsafe { entry.DriverProvider().map_err(wua_err)?.to_string() },
            512,
        ),
        driver_date_iso: unsafe {
            ole_automation_date_to_iso(entry.DriverVerDate().map_err(wua_err)?).unwrap_or_default()
        },
        device_problem_number: unsafe { entry.DeviceProblemNumber().map_err(wua_err)? },
        device_status: unsafe { entry.DeviceStatus().map_err(wua_err)? },
        min_download_bytes: common.min_download_bytes,
        max_download_bytes: common.max_download_bytes,
        target_version: common.target_version.clone(),
        target_version_source: common.target_version_source,
        support_url: common.support_url.clone(),
    })
}

fn offer_from_base(
    common: &CommonOfferMetadata,
    driver: &IWindowsDriverUpdate,
) -> Result<DriverOffer> {
    Ok(DriverOffer {
        update_id: common.update_id.clone(),
        revision: common.revision,
        title: common.title.clone(),
        hardware_id: bounded(
            unsafe { driver.DriverHardwareID().map_err(wua_err)?.to_string() },
            4096,
        ),
        driver_class: bounded(
            unsafe { driver.DriverClass().map_err(wua_err)?.to_string() },
            512,
        ),
        manufacturer: bounded(
            unsafe { driver.DriverManufacturer().map_err(wua_err)?.to_string() },
            512,
        ),
        model: bounded(
            unsafe { driver.DriverModel().map_err(wua_err)?.to_string() },
            512,
        ),
        provider: bounded(
            unsafe { driver.DriverProvider().map_err(wua_err)?.to_string() },
            512,
        ),
        driver_date_iso: unsafe {
            ole_automation_date_to_iso(driver.DriverVerDate().map_err(wua_err)?).unwrap_or_default()
        },
        device_problem_number: unsafe { driver.DeviceProblemNumber().map_err(wua_err)? },
        device_status: unsafe { driver.DeviceStatus().map_err(wua_err)? },
        min_download_bytes: common.min_download_bytes,
        max_download_bytes: common.max_download_bytes,
        target_version: common.target_version.clone(),
        target_version_source: common.target_version_source,
        support_url: common.support_url.clone(),
    })
}

fn bounded(value: String, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value
    } else {
        value.chars().take(max_chars).collect()
    }
}

unsafe fn decimal_to_u64(value: &DECIMAL) -> Result<u64> {
    #[link(name = "oleaut32")]
    unsafe extern "system" {
        fn VarUI8FromDec(pdecin: *const DECIMAL, pout: *mut u64) -> windows::core::HRESULT;
    }

    let mut out = 0u64;
    let hr = unsafe { VarUI8FromDec(value, &mut out) };
    if hr.is_err() {
        return Err(UpdateError::InvalidSize);
    }
    Ok(out)
}

fn wua_err(error: windows::core::Error) -> UpdateError {
    // WU_E_NO_CONNECTION (0x8024001F): operation could not complete because the
    // network connection was unavailable. Preserve it as truth-bearing provider state.
    if error.code().0 as u32 == 0x8024_001F {
        UpdateError::Offline(error.to_string())
    } else {
        UpdateError::Wua(error.to_string())
    }
}
