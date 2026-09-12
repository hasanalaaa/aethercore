use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KnownWindowsError {
    pub code: i64,
    pub class: String,
    pub user_message_key: String,
    pub definitive: bool,
}

pub fn classify_windows_error(code: i64) -> KnownWindowsError {
    // Keep this list intentionally narrow. Mapping a code is not, by itself, root-cause authority.
    match code as u32 {
        0x800F081F => KnownWindowsError {
            code,
            class: "SourceRequired".into(),
            user_message_key: "repair.error.sourceRequired".into(),
            definitive: true,
        },
        0x80240022 => KnownWindowsError {
            code,
            class: "WindowsUpdateFailure".into(),
            user_message_key: "repair.error.updateFailure".into(),
            definitive: false,
        },
        0x8024402C => KnownWindowsError {
            code,
            class: "NetworkOrProxy".into(),
            user_message_key: "repair.error.networkOrProxy".into(),
            definitive: false,
        },
        0x80070005 => KnownWindowsError {
            code,
            class: "AccessDenied".into(),
            user_message_key: "repair.error.accessDenied".into(),
            definitive: true,
        },
        0x80070422 => KnownWindowsError {
            code,
            class: "ServiceUnavailable".into(),
            user_message_key: "repair.error.serviceUnavailable".into(),
            definitive: false,
        },
        _ => KnownWindowsError {
            code,
            class: "UnknownWindowsError".into(),
            user_message_key: "repair.error.unknown".into(),
            definitive: false,
        },
    }
}
