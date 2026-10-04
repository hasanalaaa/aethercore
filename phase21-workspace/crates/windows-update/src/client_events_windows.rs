//! Read-only, bounded Windows Update client evidence. No event channel is enabled or changed.
use crate::{ClientErrors, ClientEvent, Result, UpdateError, parse_client_event_xml};
use std::time::{Duration, Instant};
use windows::{
    Win32::{
        Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS},
        System::EventLog::{
            EVT_HANDLE, EVT_VARIANT, EvtChannelConfigEnabled, EvtClose,
            EvtGetChannelConfigProperty, EvtNext, EvtOpenChannelConfig, EvtQuery,
            EvtQueryChannelPath, EvtQueryReverseDirection, EvtRender, EvtRenderEventXml,
            EvtVarTypeBoolean,
        },
    },
    core::PCWSTR,
};

struct EventHandle(EVT_HANDLE);
impl Drop for EventHandle {
    fn drop(&mut self) {
        if self.0 != EVT_HANDLE::default() {
            let _ = unsafe { EvtClose(self.0) };
        }
    }
}
fn event_error(error: windows::core::Error) -> UpdateError {
    UpdateError::Wua(format!("Windows Update client event log: {error}"))
}

/// Both local channels share five seconds and at most 200 records. Each gets half the cap
/// so a busy Operational log cannot hide System evidence. Partial reads retain explicit coverage.
pub fn query_client_errors(max_entries: usize) -> Result<ClientErrors> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let cap = max_entries.min(200);
    let mut output = ClientErrors::default();
    for (channel, count, filter) in [
        (
            "Microsoft-Windows-WindowsUpdateClient/Operational",
            cap.div_ceil(2),
            "Level=2",
        ),
        ("System", cap / 2, "(Level=2 or EventID=21)"),
    ] {
        match query_channel(channel, filter, count, deadline) {
            Ok(read) => {
                output.errors.extend(read.errors);
                for stamp in read.reboot_notifications {
                    if !output.reboot_notifications.contains(&stamp) {
                        output.reboot_notifications.push(stamp);
                    }
                }
                output.unknown_events += read.unknown_events;
                output.truncated |= read.truncated;
            }
            Err(_) => output.unavailable_channels += 1,
        }
    }
    Ok(output)
}

fn query_channel(
    channel: &str,
    filter: &str,
    max_entries: usize,
    deadline: Instant,
) -> Result<ClientErrors> {
    if Instant::now() >= deadline {
        return Err(UpdateError::Timeout("Windows Update client events".into()));
    }
    let channel: Vec<u16> = channel.encode_utf16().chain([0]).collect();
    let query: Vec<u16> = format!("*[System[Provider[@Name='Microsoft-Windows-WindowsUpdateClient'] and {filter} and TimeCreated[timediff(@SystemTime) >= 0 and timediff(@SystemTime) <= 2592000000]]]").encode_utf16().chain([0]).collect();
    let config = EventHandle(
        unsafe { EvtOpenChannelConfig(None, PCWSTR(channel.as_ptr()), 0) }.map_err(event_error)?,
    );
    let mut enabled = EVT_VARIANT::default();
    let mut used = 0;
    unsafe {
        EvtGetChannelConfigProperty(
            config.0,
            EvtChannelConfigEnabled,
            0,
            std::mem::size_of::<EVT_VARIANT>() as u32,
            Some(&mut enabled),
            &mut used,
        )
    }
    .map_err(event_error)?;
    if used != std::mem::size_of::<EVT_VARIANT>() as u32
        || enabled.Type != EvtVarTypeBoolean.0 as u32
        || !unsafe { enabled.Anonymous.BooleanVal }.as_bool()
    {
        return Err(UpdateError::Wua(
            "Windows Update client event channel is disabled or unknown".into(),
        ));
    }
    let result = EventHandle(
        unsafe {
            EvtQuery(
                None,
                PCWSTR(channel.as_ptr()),
                PCWSTR(query.as_ptr()),
                EvtQueryChannelPath.0 | EvtQueryReverseDirection.0,
            )
        }
        .map_err(event_error)?,
    );
    let mut output = ClientErrors::default();
    for index in 0..=max_entries {
        if Instant::now() >= deadline {
            return Err(UpdateError::Timeout("Windows Update client events".into()));
        }
        let mut handles = [0isize];
        let mut returned = 0;
        let remaining_ms = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(1000) as u32;
        if remaining_ms == 0 {
            return Err(UpdateError::Timeout("Windows Update client events".into()));
        }
        let next = unsafe { EvtNext(result.0, &mut handles, remaining_ms, 0, &mut returned) };
        // Own every returned handle before checking the API's count or any error.
        let event = EventHandle(EVT_HANDLE(handles[0]));
        match next {
            Err(error) if error.code() == ERROR_NO_MORE_ITEMS.to_hresult() => break,
            Err(error) => return Err(event_error(error)),
            Ok(()) => {}
        }
        if returned != 1 || event.0 == EVT_HANDLE::default() {
            return Err(UpdateError::Wua(
                "Windows Update client event handle count is invalid".into(),
            ));
        }
        if index == max_entries {
            output.truncated = true;
            break;
        }
        let xml = render_xml(event.0)?;
        match parse_client_event_xml(&xml) {
            Some(ClientEvent::Error(error))
                if error.unix_ms <= chrono::Utc::now().timestamp_millis() =>
            {
                output.errors.push(error)
            }
            Some(ClientEvent::RebootNotification(stamp))
                if stamp <= chrono::Utc::now().timestamp_millis() =>
            {
                output.reboot_notifications.push(stamp)
            }
            _ => output.unknown_events += 1,
        }
    }
    if Instant::now() >= deadline {
        return Err(UpdateError::Timeout("Windows Update client events".into()));
    }
    Ok(output)
}

fn render_xml(event: EVT_HANDLE) -> Result<String> {
    const MAX_BYTES: u32 = 64 * 1024;
    let (mut used, mut properties) = (0, 0);
    let first = unsafe {
        EvtRender(
            None,
            event,
            EvtRenderEventXml.0,
            0,
            None,
            &mut used,
            &mut properties,
        )
    };
    if let Err(error) = first
        && error.code() != ERROR_INSUFFICIENT_BUFFER.to_hresult()
    {
        return Err(event_error(error));
    }
    if used == 0 || used > MAX_BYTES || used % 2 != 0 {
        return Err(UpdateError::Wua(
            "Windows Update client event XML exceeds its size bound".into(),
        ));
    }
    let capacity = used;
    let mut buffer = vec![0u16; (capacity / 2) as usize];
    unsafe {
        EvtRender(
            None,
            event,
            EvtRenderEventXml.0,
            capacity,
            Some(buffer.as_mut_ptr().cast()),
            &mut used,
            &mut properties,
        )
    }
    .map_err(event_error)?;
    if used > capacity || used % 2 != 0 {
        return Err(UpdateError::Wua(
            "Windows Update client event XML has an invalid length".into(),
        ));
    }
    let buffer = &buffer[..(used / 2) as usize];
    let len = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    String::from_utf16(&buffer[..len])
        .map_err(|_| UpdateError::Wua("Windows Update client event XML is not UTF-16".into()))
}
