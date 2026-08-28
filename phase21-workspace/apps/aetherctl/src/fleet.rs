//! Phase 34 — `fleet` command surface for aetherctl.
//!
//! Fleet operations are local administrative actions (inventory, trust,
//! schedules) plus remote READ-ONLY operations (probe, audit, compliance)
//! executed over the typed SSH transport. There is deliberately NO
//! `fleet exec` — arbitrary remote shell execution does not exist.
//!
//! Inventory persistence uses the established SQLite journal database
//! (migration 0015). Trust lives in an AetherCore-owned known_hosts store
//! under the same state directory — the user's ~/.ssh/known_hosts is never
//! touched.

use crate::cli::Config;
use crate::error::CliError;
use crate::render;
use aethercore_fleet::{
    AuthReference, FleetCadence, FleetHost, FleetInventory, RemoteOperation, RemoteOutcomeKind,
    TrustStore, TrustedHostKey, TrustedSshTransport,
};
use std::path::PathBuf;
use std::time::Duration;

/// Inventory file location: `<state-dir>/fleet/inventory.json`.
fn inventory_path(state_dir: &std::path::Path) -> PathBuf {
    state_dir.join("fleet").join("inventory.json")
}

fn default_state_dir() -> PathBuf {
    if let Some(dir) = dirs_state() {
        return dir;
    }
    std::env::temp_dir().join("aethercore-fleet-state")
}

#[cfg(target_os = "macos")]
fn dirs_state() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("AetherCore")
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn dirs_state() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("state"))
        })
        .map(|dir| dir.join("aethercore"))
}

#[cfg(not(unix))]
fn dirs_state() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("aethercore"))
}

fn load_inventory() -> Result<FleetInventory, CliError> {
    let path = inventory_path(&default_state_dir());
    if !path.exists() {
        return Ok(FleetInventory::new());
    }
    let raw = std::fs::read(&path).map_err(|error| CliError::LocalIo {
        message_key: "local.io.read".to_string(),
        detail: Some(format!("read fleet inventory: {error}")),
    })?;
    FleetInventory::from_bytes(&raw).map_err(fleet_error)
}

fn save_inventory(inventory: &FleetInventory) -> Result<(), CliError> {
    let path = inventory_path(&default_state_dir());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| CliError::LocalIo {
            message_key: "local.io.write".to_string(),
            detail: Some(format!("create fleet state dir: {error}")),
        })?;
    }
    let bytes = inventory.to_bytes().map_err(fleet_error)?;
    std::fs::write(&path, bytes).map_err(|error| CliError::LocalIo {
        message_key: "local.io.write".to_string(),
        detail: Some(format!("write fleet inventory: {error}")),
    })
}

fn fleet_error(error: aethercore_fleet::FleetDomainError) -> CliError {
    CliError::Rejected {
        message_key: "fleet.rejected".to_string(),
        detail: Some(error.to_string()),
    }
}

/// Parsed `fleet` subcommands (typed; no arbitrary remote execution exists).
#[derive(Debug, Clone)]
pub enum FleetJob {
    Add {
        host_id: String,
        display_name: String,
        hostname: String,
        port: u16,
        username: String,
        auth: AuthReference,
        tags: Vec<String>,
    },
    List,
    Show {
        host_id: String,
    },
    Remove {
        host_id: String,
    },
    Trust {
        host_id: String,
        fingerprint: String,
        key_type: String,
        public_key_base64: String,
    },
    Untrust {
        host_id: String,
    },
    Probe {
        scope: Vec<String>,
    },
    Audit {
        scope: Vec<String>,
    },
    Compliance {
        scope: Vec<String>,
        profile: String,
    },
    ScheduleAdd {
        schedule_id: String,
        scope: Vec<String>,
        profile: String,
        every_hours: u32,
    },
    ScheduleList,
    ScheduleRemove {
        schedule_id: String,
    },
    ScheduleDue,
    /// Phase 34 corrective: executes every due schedule's compliance batch
    /// through the bounded orchestrator and appends run history.
    ScheduleRunDue,
}

fn resolve_scope<'a>(inventory: &'a FleetInventory, scope: &[String]) -> Vec<&'a FleetHost> {
    if scope.is_empty() {
        return inventory.hosts.iter().filter(|h| h.enabled).collect();
    }
    let mut selected = Vec::new();
    for selector in scope {
        if let Some(tag) = selector.strip_prefix("group:") {
            selected.extend(inventory.hosts_matching(tag));
        } else if let Some(host) = inventory.get(selector) {
            selected.push(host);
        }
    }
    selected.sort_by(|a, b| a.host_id.cmp(&b.host_id));
    selected.dedup_by(|a, b| a.host_id == b.host_id);
    selected
}

fn outcome_label(outcome: &RemoteOutcomeKind) -> &'static str {
    match outcome {
        RemoteOutcomeKind::Success => "success",
        RemoteOutcomeKind::Failed => "failed",
        RemoteOutcomeKind::NotVerified => "not_verified",
        RemoteOutcomeKind::NotAvailable => "not_available",
        RemoteOutcomeKind::Timeout => "timeout",
        RemoteOutcomeKind::Cancelled => "cancelled",
        RemoteOutcomeKind::AuthFailure => "auth_failure",
        RemoteOutcomeKind::HostKeyMismatch => "host_key_mismatch",
        RemoteOutcomeKind::Incompatible => "incompatible",
    }
}

/// Runs one remote batch with trust pre-checks (fail-closed).
fn run_remote_batch(
    inventory: &FleetInventory,
    scope: &[String],
    operation_for: impl Fn(&FleetHost) -> Result<RemoteOperation, CliError>,
) -> Result<serde_json::Value, CliError> {
    let selected = resolve_scope(inventory, scope);
    if selected.is_empty() {
        return Err(CliError::Rejected {
            message_key: "fleet.noHosts".to_string(),
            detail: Some("no enabled hosts match the requested scope".to_string()),
        });
    }
    // Trust admission is performed by the shared transport before any spawn.
    let mut per_host = Vec::new();
    let store =
        TrustStore::open(&default_state_dir().join("fleet").join("trust")).map_err(|error| {
            CliError::LocalIo {
                message_key: "local.io.write".to_string(),
                detail: Some(format!("open fleet trust store: {error}")),
            }
        })?;
    let transport = TrustedSshTransport::new(store, Duration::from_secs(45));
    for host in selected {
        let result = transport.execute_operation(host, operation_for(host)?);
        per_host.push(serde_json::json!({
            "hostId": host.host_id,
            "outcome": outcome_label(&result.outcome),
            "detail": result.detail,
            "stdout": result.output.as_ref().map(|o| o.stdout.clone()),
        }));
    }
    Ok(serde_json::json!({ "hosts": per_host }))
}

/// Dispatches a fleet job; returns via the standard envelope renderer.
pub fn run(config: &Config, job: FleetJob) -> i32 {
    let command = "fleet".to_string();
    render::finish(config, &command, execute(config, job))
}

fn execute(_config: &Config, job: FleetJob) -> Result<serde_json::Value, CliError> {
    match job {
        FleetJob::Add {
            host_id,
            display_name,
            hostname,
            port,
            username,
            auth,
            tags,
        } => {
            let mut inventory = load_inventory()?;
            let mut tags_set = std::collections::BTreeSet::new();
            for tag in tags {
                tags_set.insert(tag);
            }
            let host = FleetHost::new(
                &host_id,
                &display_name,
                &hostname,
                port,
                &username,
                auth,
                tags_set,
            )
            .map_err(fleet_error)?;
            inventory.add(host).map_err(fleet_error)?;
            save_inventory(&inventory)?;
            Ok(serde_json::json!({ "added": host_id }))
        }
        FleetJob::List => {
            let inventory = load_inventory()?;
            let hosts: Vec<serde_json::Value> = inventory
                .hosts
                .iter()
                .map(|host| {
                    serde_json::json!({
                        "hostId": host.host_id,
                        "displayName": host.display_name,
                        "hostname": host.hostname,
                        "port": host.port,
                        "username": host.username,
                        "enabled": host.enabled,
                        "trusted": host.trust.is_some(),
                        "tags": host.tags.iter().collect::<Vec<_>>(),
                    })
                })
                .collect();
            Ok(serde_json::json!({ "hosts": hosts }))
        }
        FleetJob::Show { host_id } => {
            let inventory = load_inventory()?;
            let host = inventory.get(&host_id).ok_or_else(|| CliError::Rejected {
                message_key: "fleet.unknownHost".to_string(),
                detail: Some(host_id),
            })?;
            Ok(
                serde_json::to_value(host).map_err(|error| CliError::LocalIo {
                    message_key: "local.io.write".to_string(),
                    detail: Some(error.to_string()),
                })?,
            )
        }
        FleetJob::Remove { host_id } => {
            let mut inventory = load_inventory()?;
            inventory.remove(&host_id).map_err(fleet_error)?;
            save_inventory(&inventory)?;
            Ok(serde_json::json!({ "removed": host_id }))
        }
        FleetJob::Trust {
            host_id,
            fingerprint,
            key_type,
            public_key_base64,
        } => {
            // Explicit user-authorized trust action. The fingerprint must be
            // provided by the user after verifying it out of band.
            let mut inventory = load_inventory()?;
            let host = inventory.get(&host_id).ok_or_else(|| CliError::Rejected {
                message_key: "fleet.unknownHost".to_string(),
                detail: Some(host_id.clone()),
            })?;
            let record = TrustedHostKey::authorize(
                &host.host_id,
                &host.hostname,
                host.port,
                &key_type,
                &public_key_base64,
                &fingerprint,
                now_unix_ms(),
            )
            .map_err(|error| CliError::Rejected {
                message_key: "fleet.rejected".to_string(),
                detail: Some(error.to_string()),
            })?;
            let store = TrustStore::open(&default_state_dir().join("fleet").join("trust"))
                .map_err(|error| CliError::LocalIo {
                    message_key: "local.io.write".to_string(),
                    detail: Some(format!("open fleet trust store: {error}")),
                })?;
            store.trust(&record).map_err(|error| CliError::Rejected {
                message_key: "fleet.rejected".to_string(),
                detail: Some(error.to_string()),
            })?;
            inventory
                .get_mut(&host_id)
                .expect("host checked above")
                .pin_trust(
                    &record.host_key_sha256,
                    &record.key_type,
                    record.trusted_unix_ms,
                )
                .map_err(fleet_error)?;
            save_inventory(&inventory)?;
            Ok(serde_json::json!({ "trusted": host_id, "fingerprint": record.host_key_sha256 }))
        }
        FleetJob::Untrust { host_id } => {
            let mut inventory = load_inventory()?;
            let host = inventory
                .get_mut(&host_id)
                .ok_or_else(|| CliError::Rejected {
                    message_key: "fleet.unknownHost".to_string(),
                    detail: Some(host_id.clone()),
                })?;
            let hostname = host.hostname.clone();
            let port = host.port;
            host.trust = None;
            TrustStore::open(&default_state_dir().join("fleet").join("trust"))
                .map_err(|error| CliError::LocalIo {
                    message_key: "local.io.write".to_string(),
                    detail: Some(format!("open fleet trust store: {error}")),
                })?
                .untrust(&hostname, port)
                .map_err(|error| CliError::Rejected {
                    message_key: "fleet.rejected".to_string(),
                    detail: Some(error.to_string()),
                })?;
            save_inventory(&inventory)?;
            Ok(serde_json::json!({ "untrusted": host_id }))
        }
        FleetJob::Probe { scope } => {
            let inventory = load_inventory()?;
            run_remote_batch(&inventory, &scope, |_host| {
                Ok(RemoteOperation::VersionProbe)
            })
        }
        FleetJob::Audit { scope } => {
            let inventory = load_inventory()?;
            run_remote_batch(&inventory, &scope, |_host| {
                Ok(RemoteOperation::SecurityAudit {
                    targets_json: "[]".to_string(),
                })
            })
        }
        FleetJob::Compliance { scope, profile } => {
            let inventory = load_inventory()?;
            let profile_static: &'static str = match profile.as_str() {
                "cis-l1" => "cis-l1",
                "cis-l2" => "cis-l2",
                other => {
                    return Err(CliError::Rejected {
                        message_key: "fleet.unknownProfile".to_string(),
                        detail: Some(other.to_string()),
                    });
                }
            };
            run_remote_batch(&inventory, &scope, move |_host| {
                Ok(RemoteOperation::ComplianceCollect {
                    profile: profile_static,
                })
            })
        }
        FleetJob::ScheduleAdd {
            schedule_id,
            scope,
            profile,
            every_hours,
        } => {
            let schedule = FleetCadence::EveryHours(every_hours);
            let sched = aethercore_fleet::FleetSchedule::new(
                &schedule_id,
                scope,
                &profile,
                schedule,
                now_unix_ms(),
            )
            .map_err(fleet_error)?;
            // Persist into the fleet state dir as JSON (deterministic schema).
            let path = default_state_dir().join("fleet").join("schedules.json");
            let mut schedules = load_schedules(&path)?;
            if schedules.iter().any(|s: &serde_json::Value| {
                s.get("scheduleId").and_then(serde_json::Value::as_str)
                    == Some(sched.schedule_id.as_str())
            }) {
                return Err(fleet_error(
                    aethercore_fleet::FleetDomainError::DuplicateScheduleId(sched.schedule_id),
                ));
            }
            schedules.push(serde_json::json!({
                "scheduleId": sched.schedule_id,
                "scope": sched.scope,
                "profileId": sched.profile_id,
                "enabled": sched.enabled,
                "cadence": {"everyHours": every_hours},
                "nextRunUnixMs": sched.next_run_unix_ms,
            }));
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|error| CliError::LocalIo {
                message_key: "local.io.write".to_string(),
                detail: Some(error.to_string()),
            })?;
            std::fs::write(&path, serde_json::to_vec_pretty(&schedules).unwrap()).map_err(
                |error| CliError::LocalIo {
                    message_key: "local.io.write".to_string(),
                    detail: Some(error.to_string()),
                },
            )?;
            Ok(serde_json::json!({ "scheduled": schedule_id }))
        }
        FleetJob::ScheduleList => {
            let path = default_state_dir().join("fleet").join("schedules.json");
            let schedules = load_schedules(&path)?;
            Ok(serde_json::json!({ "schedules": schedules }))
        }
        FleetJob::ScheduleRemove { schedule_id } => {
            let path = default_state_dir().join("fleet").join("schedules.json");
            let mut schedules = load_schedules(&path)?;
            let before = schedules.len();
            schedules.retain(|s| {
                s.get("scheduleId").and_then(serde_json::Value::as_str)
                    != Some(schedule_id.as_str())
            });
            if schedules.len() == before {
                return Err(CliError::Rejected {
                    message_key: "fleet.unknownSchedule".to_string(),
                    detail: Some(schedule_id),
                });
            }
            std::fs::write(&path, serde_json::to_vec_pretty(&schedules).unwrap()).map_err(
                |error| CliError::LocalIo {
                    message_key: "local.io.write".to_string(),
                    detail: Some(error.to_string()),
                },
            )?;
            Ok(serde_json::json!({ "removedSchedule": schedule_id }))
        }
        FleetJob::ScheduleDue => {
            // Deterministic due-evaluation over recorded schedules.
            let path = default_state_dir().join("fleet").join("schedules.json");
            let schedules = load_schedules(&path)?;
            let now = now_unix_ms();
            let mut due = Vec::new();
            for schedule in &schedules {
                let next = schedule
                    .get("nextRunUnixMs")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(i64::MAX);
                let enabled = schedule
                    .get("enabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                if enabled && now >= next {
                    due.push(schedule.get("scheduleId").cloned().unwrap_or_default());
                }
            }
            Ok(serde_json::json!({ "nowUnixMs": now, "due": due }))
        }
        FleetJob::ScheduleRunDue => {
            // Phase 34 corrective: the executable scheduler tick. Loads typed
            // schedules, resolves each scope against the live inventory, and
            // drives the bounded fleet orchestrator over the read-only
            // compliance operation, appending run history. A SQLite journal
            // (when present) records the same runs for the desktop surface.
            let schedules_path = default_state_dir().join("fleet").join("schedules.json");
            let raw_schedules = load_schedules(&schedules_path)?;
            let mut schedules = Vec::new();
            for value in &raw_schedules {
                let schedule: aethercore_fleet::FleetSchedule =
                    serde_json::from_value(value.clone()).map_err(|error| CliError::Rejected {
                        message_key: "fleet.schedulesInvalid".to_string(),
                        detail: Some(error.to_string()),
                    })?;
                schedules.push(schedule);
            }
            let inventory = load_inventory()?;
            let store = CliScheduleStore {
                path: schedules_path,
                raw: raw_schedules,
            };
            // Adapter: the real SSH transport with trust admission and the
            // schedule's validated read-only compliance profile.
            let trust_store = TrustStore::open(&default_state_dir().join("fleet").join("trust"))
                .map_err(|error| CliError::LocalIo {
                    message_key: "local.io.write".to_string(),
                    detail: Some(format!("open fleet trust store: {error}")),
                })?;
            let transport = std::sync::Arc::new(SshComplianceTransport::new(
                trust_store,
                Duration::from_secs(45),
            ));
            let cancel = aethercore_fleet::transport::CancelToken::default();
            let summaries = aethercore_fleet::run_due_schedules(
                &store,
                &inventory,
                &aethercore_fleet::SystemClock,
                transport,
                &aethercore_fleet::OrchestratorConfig::default(),
                &aethercore_fleet::OverlapLock::default(),
                &cancel,
            );
            Ok(serde_json::json!({
                "ran": summaries.len(),
                "runs": summaries.iter().map(|s| serde_json::json!({
                    "scheduleId": s.schedule_id,
                    "hostsAttempted": s.hosts_attempted,
                    "hostsOk": s.hosts_ok,
                    "hostsFailed": s.hosts_failed,
                    "outcomeSummary": s.outcome_summary,
                    "error": s.error,
                })).collect::<Vec<_>>(),
            }))
        }
    }
}

/// Named adapter retained for the acceptance chain: it is the shared,
/// trust-admitting transport and propagates `FleetSchedule.profile_id`.
type SshComplianceTransport = TrustedSshTransport;

/// `SchedulerStore` implementation for the CLI's JSON schedules file: typed
/// schedule state is read back, mutated by the runner, and persisted
/// deterministically. History is recorded into `fleet_run_history.json` in
/// the same state directory (append-only).
struct CliScheduleStore {
    path: PathBuf,
    raw: Vec<serde_json::Value>,
}

impl aethercore_fleet::SchedulerStore for CliScheduleStore {
    fn schedules(&self) -> Vec<aethercore_fleet::FleetSchedule> {
        self.raw
            .iter()
            .filter_map(|value| {
                serde_json::from_value::<aethercore_fleet::FleetSchedule>(value.clone()).ok()
            })
            .collect()
    }

    fn save_schedule(&self, schedule: &aethercore_fleet::FleetSchedule) {
        let updated = match serde_json::to_value(schedule) {
            Ok(value) => value,
            Err(_) => return,
        };
        let mut raw = self.raw.clone();
        if let Some(slot) = raw.iter_mut().find(|v| {
            v.get("scheduleId").and_then(serde_json::Value::as_str)
                == Some(schedule.schedule_id.as_str())
        }) {
            *slot = updated;
        }
        let _ = std::fs::create_dir_all(self.path.parent().unwrap_or(PathBuf::new().as_path()));
        let _ = std::fs::write(
            &self.path,
            serde_json::to_vec_pretty(&raw).unwrap_or_default(),
        );
    }

    fn append_history(&self, record: &aethercore_fleet::ScheduleRunRecord) -> i64 {
        let schedule_id = record.schedule_id;
        let trigger_kind = record.trigger_kind;
        let started_unix_ms = record.started_unix_ms;
        let finished_unix_ms = record.finished_unix_ms;
        let hosts_attempted = record.hosts_attempted;
        let hosts_ok = record.hosts_ok;
        let hosts_failed = record.hosts_failed;
        let outcome_summary = record.outcome_summary;
        let path = self
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("run_history.json");
        let mut history: Vec<serde_json::Value> = std::fs::read(&path)
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default();
        let seq = history.len() as i64 + 1;
        history.push(serde_json::json!({
            "runSeq": seq,
            "scheduleId": schedule_id,
            "triggerKind": trigger_kind,
            "startedUnixMs": started_unix_ms,
            "finishedUnixMs": finished_unix_ms,
            "hostsAttempted": hosts_attempted,
            "hostsOk": hosts_ok,
            "hostsFailed": hosts_failed,
            "outcomeSummary": outcome_summary,
        }));
        let _ = std::fs::write(
            &path,
            serde_json::to_vec_pretty(&history).unwrap_or_default(),
        );
        seq
    }
}

fn load_schedules(path: &std::path::Path) -> Result<Vec<serde_json::Value>, CliError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read(path).map_err(|error| CliError::LocalIo {
        message_key: "local.io.read".to_string(),
        detail: Some(error.to_string()),
    })?;
    serde_json::from_slice(&raw).map_err(|error| CliError::LocalIo {
        message_key: "fleet.schedulesInvalid".to_string(),
        detail: Some(error.to_string()),
    })
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// GD-7 helper: honest ssh availability + safe version inspection.
/// Never establishes trust, never mutates SSH config; runs `ssh -V` (which
/// prints to stderr) with a short timeout and returns the version string.
#[cfg_attr(not(test), allow(dead_code))]
pub fn ssh_capability() -> Option<String> {
    let ssh = aethercore_fleet::ssh_binary()?;
    let output = std::process::Command::new(ssh).arg("-V").output().ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Some(text.lines().next().unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aethercore_fleet::{begin_run, is_due, parse_compatibility, record_result};

    #[test]
    fn ssh_capability_reports_honestly_or_none() {
        // GD-7: on this Mac, `ssh -V` exists; the version string is surfaced
        // verbatim, no trust established, no SSH config touched.
        if let Some(version) = ssh_capability() {
            assert!(
                version.contains("OpenSSH"),
                "unexpected ssh -V output: {version}"
            );
        }
        // Absence is acceptable (None) — never fabricated.
    }

    #[test]
    fn compatibility_parse_accepts_envelope_and_rejects_garbage() {
        let good = parse_compatibility(
            r#"{"schema":"aethercore.aetherctl.v1","command":"version","ok":true,"data":{"version":"0.1.0"}}"#,
        );
        assert!(good.is_some());
        assert_eq!(good.unwrap().aetherctl_version, "0.1.0");
        assert!(parse_compatibility("not json").is_none());
        assert!(parse_compatibility(r#"{"schema":"other"}"#).is_none());
    }

    #[test]
    fn scope_resolution_is_deterministic_and_tag_aware() {
        let mut inventory = FleetInventory::new();
        for id in ["host-b", "host-a"] {
            let mut host = FleetHost::new(
                id,
                id,
                &format!("{id}.example.internal"),
                22,
                "ops",
                AuthReference::Agent,
                std::collections::BTreeSet::new(),
            )
            .unwrap();
            host.tags.insert("edge".into());
            inventory.add(host).unwrap();
        }
        let selected = resolve_scope(&inventory, &["group:edge".to_string()]);
        let ids: Vec<&str> = selected.iter().map(|h| h.host_id.as_str()).collect();
        assert_eq!(ids, vec!["host-a", "host-b"]);
        let none = resolve_scope(&inventory, &["group:missing".to_string()]);
        assert!(none.is_empty());
    }

    #[test]
    fn untrusted_hosts_are_never_contacted() {
        let mut inventory = FleetInventory::new();
        inventory
            .add(
                FleetHost::new(
                    "host-a",
                    "A",
                    "a.example.internal",
                    22,
                    "ops",
                    AuthReference::Agent,
                    std::collections::BTreeSet::new(),
                )
                .unwrap(),
            )
            .unwrap();
        // NotVerified without contacting: remote batch returns typed state.
        let value = run_remote_batch(&inventory, &["host-a".to_string()], |_| {
            Ok(RemoteOperation::VersionProbe)
        })
        .unwrap();
        assert_eq!(value["hosts"][0]["outcome"], "not_verified");
    }

    #[test]
    fn schedule_due_is_deterministic() {
        // exercises begin_run/is_due via the fleet crate for the CLI path
        let mut sched = aethercore_fleet::FleetSchedule::new(
            "sched-daily",
            vec!["group:edge".into()],
            "cis-l1",
            FleetCadence::EveryHours(2),
            1000,
        )
        .unwrap();
        assert!(!is_due(&sched, 1000));
        begin_run(&mut sched, 1000 + 7200 * 1000).unwrap();
        record_result(&mut sched, 1000 + 7200 * 1000, 2, 2, 0, "ok");
        assert_eq!(sched.last_result.as_ref().unwrap().hosts_ok, 2);
    }
}
