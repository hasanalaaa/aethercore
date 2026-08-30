use aethercore_support_bundle::SupportSection;
use serde_json::json;

use crate::router::ServiceContext;

pub(crate) fn build_sections(
    ctx: &ServiceContext,
    owner: &str,
    include_hardware: bool,
    include_crash_metadata: bool,
    include_operation_history: bool,
    include_scheduler_activity: bool,
) -> Result<Vec<SupportSection>, crate::errors::ServiceError> {
    let mut sections = Vec::new();
    let event_bus = ctx.kernel.events().metrics_for_owner(owner);
    sections.push(SupportSection {
        file_name: "product.json".into(),
        display_key: "support.section.product".into(),
        value: json!({
            "product": "AetherCore",
            "version": env!("CARGO_PKG_VERSION"),
            "protocolVersion": aethercore_contracts::PROTOCOL_VERSION,
            "runtime": {
                "eventBus": {
                    "publishedTotal": event_bus.published_total,
                    "subscriberLagTotal": event_bus.subscriber_lag_total,
                    "subscriberDisconnectTotal": event_bus.subscriber_disconnect_total,
                    "subscribers": event_bus.subscribers,
                    "replayEvents": event_bus.replay_events,
                }
            }
        }),
    });

    if include_hardware || include_crash_metadata {
        if let Ok(mut snapshot) = ctx.diagnostics.snapshot_for_owner(owner) {
            if !include_hardware {
                snapshot.storage.clear();
                snapshot.memory = None;
            }
            if !include_crash_metadata {
                snapshot.events.clear();
                snapshot.crashes.clear();
            }
            // Diagnostic cards may combine hardware and crash evidence. Keep the original card set only
            // when both domains were requested; otherwise expose the bounded raw evidence selected above.
            if !(include_hardware && include_crash_metadata) {
                snapshot.cards.clear();
            }
            sections.push(SupportSection {
                file_name: "diagnostics.json".into(),
                display_key: "support.section.diagnostics".into(),
                value: serde_json::to_value(snapshot).map_err(|e| crate::errors::ServiceError::internal(
                    "support-bundle", "support.error.serialization", e.to_string()))?,
            });
        }
    }

    if include_operation_history {
        let events = ctx.db.support_journal_events_for_owner(owner, 200)
            .map_err(|e| crate::errors::ServiceError::internal("support-bundle", "support.error.persistence", e.to_string()))?;
        sections.push(SupportSection {
            file_name: "operation-history.json".into(),
            display_key: "support.section.operationHistory".into(),
            value: serde_json::to_value(events).map_err(|e| crate::errors::ServiceError::internal(
                "support-bundle", "support.error.serialization", e.to_string()))?,
        });
    }

    if include_scheduler_activity {
        let runs = ctx.db.scheduler_runs_for_owner(owner)
            .map_err(|e| crate::errors::ServiceError::internal("support-bundle", "support.error.persistence", e.to_string()))?;
        let values: Vec<_> = runs.into_iter().map(|run| json!({
            "workload": run.workload,
            "failureCount": run.failure_count,
            "nextEligibleUnixMs": run.next_eligible_unix_ms,
            "lastOutcome": run.last_outcome,
            "lastCompletedUnixMs": run.last_completed_unix_ms,
            "updatedUnixMs": run.updated_unix_ms,
        })).collect();
        sections.push(SupportSection {
            file_name: "scheduler-activity.json".into(),
            display_key: "support.section.schedulerActivity".into(),
            value: serde_json::Value::Array(values),
        });
    }

    Ok(sections)
}
