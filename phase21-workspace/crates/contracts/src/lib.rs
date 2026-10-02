#![forbid(unsafe_code)]

pub const PROTOCOL_VERSION: u32 = 7;

// Requests are intentionally tiny: every operation is a typed identifier/request rather than an
// arbitrary command or payload. Driver inventory responses can legitimately be much larger on
// systems with extensive PnP trees, so response capacity is bounded independently.
pub const MAX_REQUEST_ID_BYTES: usize = 128;
pub const MAX_REQUEST_FRAME_BYTES: usize = 256 * 1024;
pub const MAX_RESPONSE_FRAME_BYTES: usize = 8 * 1024 * 1024;
// Client session frames carry only bounded RPC/control envelopes; they do not need the multi-MiB
// allowance required for large device/diagnostic responses sent by the service.
pub const MAX_CLIENT_SESSION_FRAME_BYTES: usize = 384 * 1024;
pub const MAX_SERVER_SESSION_FRAME_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_REQUEST_DEADLINE_MS: i64 = 15_000;
pub const MAX_REQUEST_DEADLINE_MS: i64 = 120_000;

pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/aethercore.v1.rs"));
}

#[cfg(test)]
mod locale_tests {
    use super::v1::ListInsightsRequest;
    use prost::Message;
    #[test]
    fn insight_list_locale_preserves_old_empty_messages_and_uses_additive_tag_one() {
        assert_eq!(ListInsightsRequest::decode(&[][..]).unwrap().locale, None);
        let arabic = ListInsightsRequest {
            locale: Some("ar".into()),
        };
        assert_eq!(arabic.encode_to_vec(), b"\x0a\x02ar");
        assert_eq!(
            ListInsightsRequest::decode(arabic.encode_to_vec().as_slice())
                .unwrap()
                .locale
                .as_deref(),
            Some("ar")
        );
    }
}

#[cfg(test)]
mod timeline_compatibility {
    use super::v1::{GetTimelinePageRequest, TimelineResponse};
    use prost::Message;
    #[test]
    fn additive_timeline_cursor_preserves_legacy_presence_and_response_defaults() {
        let old = GetTimelinePageRequest::decode(&b"\x08\x03\x10\x02"[..]).unwrap();
        assert_eq!(old.page_size, 3);
        assert_eq!(old.before_sequence, 2);
        assert!(old.snapshot_cursor.is_none());
        assert_eq!(old.encode_to_vec(), b"\x08\x03\x10\x02");
        let initial = GetTimelinePageRequest {
            snapshot_cursor: Some(String::new()),
            ..Default::default()
        };
        assert_eq!(initial.encode_to_vec(), b"\x1a\x00");
        assert_eq!(
            GetTimelinePageRequest::decode(initial.encode_to_vec().as_slice())
                .unwrap()
                .snapshot_cursor
                .as_deref(),
            Some("")
        );
        let old_response = TimelineResponse::decode(&[][..]).unwrap();
        assert!(!old_response.reload_required && old_response.next_snapshot_cursor.is_none());
        assert_eq!(
            TimelineResponse {
                reload_required: true,
                ..Default::default()
            }
            .encode_to_vec(),
            b"\x40\x01"
        );
    }
}
