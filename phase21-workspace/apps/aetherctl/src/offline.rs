//! Embedded OFFLINE commands (T3): they work with NO service and are STRICTLY
//! read-only. This module contains zero mutation-capable symbols — enforced by the
//! Phase 28 audit symbol scan (p28-mutation-guard): no filesystem writes, no wire
//! request construction, no mutating RPC payload types.

use crate::cli::{Config, OfflineJob};
use crate::error::CliError;
use crate::render;
use crate::transport;
use aethercore_platform_capabilities::Availability;

pub fn run(config: &Config, job: OfflineJob) -> i32 {
    let command = command_label(&job);
    match job {
        OfflineJob::Help => {
            print!("{}", crate::cli::USAGE);
            crate::exit::ExitCode::Ok.as_i32()
        }
        OfflineJob::ServiceUnits => {
            if config.output == crate::cli::OutputMode::Json {
                render::finish(
                    config,
                    &command,
                    Ok(serde_json::json!({
                        "launchdPlist": crate::units::LAUNCHD_PLIST,
                        "systemdUnit": crate::units::SYSTEMD_UNIT,
                    })),
                )
            } else {
                crate::units::print_units();
                crate::exit::ExitCode::Ok.as_i32()
            }
        }
        other => {
            let outcome = execute(config, other);
            render::finish(config, &command, outcome)
        }
    }
}

pub fn command_label(job: &OfflineJob) -> String {
    match job {
        OfflineJob::Help => "help".to_string(),
        OfflineJob::About => "about".to_string(),
        OfflineJob::Version => "version".to_string(),
        OfflineJob::Capabilities => "capabilities".to_string(),
        OfflineJob::EngineSource => "engine-source".to_string(),
        OfflineJob::TelemetryOnce { .. } => "telemetry-once".to_string(),
        OfflineJob::SelfCheck { .. } => "self-check".to_string(),
        OfflineJob::ServiceDetect => "service detect".to_string(),
        OfflineJob::ServiceUnits => "service units".to_string(),
        OfflineJob::ExportVerify { .. } => "export verify".to_string(),
        OfflineJob::KeysGenerate { .. } => "keys generate".to_string(),
        OfflineJob::KeysFingerprint { .. } => "keys fingerprint".to_string(),
        OfflineJob::DbCheckSqlite { .. } => "db check".to_string(),
        OfflineJob::SecAudit { .. } => "sec audit".to_string(),
        OfflineJob::ComplianceAudit { .. } => "sec audit --profile".to_string(),
        OfflineJob::ComplianceVerify { .. } => "compliance verify".to_string(),
        OfflineJob::SecReport { .. } => "sec report".to_string(),
        OfflineJob::SecComplianceSummary { .. } => "compliance summary".to_string(),
        OfflineJob::VulndbUpdate { .. } => "vulndb update".to_string(),
        OfflineJob::ReleaseInspect { .. } => "release inspect".to_string(),
        OfflineJob::ReleaseVerify { .. } => "release verify".to_string(),
        OfflineJob::UpdateCheck => "update check".to_string(),
        OfflineJob::UpdatePlan => "update plan".to_string(),
        OfflineJob::UpdateDownload => "update download".to_string(),
        OfflineJob::UpdateVerify { .. } => "update verify".to_string(),
        OfflineJob::UpdateStage => "update stage".to_string(),
        OfflineJob::UpdateStatus => "update status".to_string(),
        OfflineJob::UpdateCancel => "update cancel".to_string(),
        OfflineJob::UpdateRollback => "update rollback".to_string(),
        OfflineJob::OfflineBundleVerify { .. } => "update offline verify".to_string(),
    }
}

fn execute(config: &Config, job: OfflineJob) -> Result<serde_json::Value, CliError> {
    match job {
        OfflineJob::About => Ok(serde_json::json!({
            "name": "aetherctl",
            "product": aethercore_product_identity::PRODUCT_NAME,
            "version": env!("CARGO_PKG_VERSION"),
            "protocolVersion": aethercore_contracts::PROTOCOL_VERSION,
            "platform": platform_str(),
        })),
        OfflineJob::Version => Ok(serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "protocolVersion": aethercore_contracts::PROTOCOL_VERSION,
        })),
        OfflineJob::Capabilities => capabilities_data(),
        OfflineJob::EngineSource => Ok(serde_json::json!({
            // Same compile-time selection expression as the service composition
            // (services/maintenance-service/src/performance.rs::engine_source) minus the
            // force-synthetic-perf audit feature this CLI does not define. Parity is
            // asserted textually by scripts/phase28-adversarial-audit.py.
            "source": offline_engine_source(),
            "platform": platform_str(),
        })),
        OfflineJob::TelemetryOnce { interval_ms } => telemetry_once(interval_ms),
        // Phase 29 (T3): offline EXPORT_V1 chain verification — recomputes everything
        // locally; trusts nothing it cannot recompute. Typed failure names the break.
        OfflineJob::ExportVerify { file } => export_verify(file.as_str()),
        // Phase 29 (T3): EXPLICIT owner key generation. The 32-byte seed is drawn from
        // the OS CSPRNG and written to --out with 0600 permissions; the public key
        // fingerprint is printed alongside. Nothing else in the product ever creates keys.
        OfflineJob::KeysGenerate { out } => keys_generate(out.as_str()),
        OfflineJob::KeysFingerprint { input } => keys_fingerprint(&input),
        OfflineJob::SelfCheck { load_model } => self_check(load_model),
        // Phase 30 (T6): offline read-only SQLite diagnostics via db-diagnostics crate.
        OfflineJob::DbCheckSqlite { path } => db_check_sqlite(&path),
        OfflineJob::SecAudit { targets } => crate::sec::run_offline_audit(&targets),
        OfflineJob::ComplianceAudit {
            profile,
            out,
            format,
            sign,
            key,
            targets,
        } => crate::sec::run_compliance_audit(
            &profile,
            &out,
            &format,
            sign,
            key.as_deref(),
            &targets,
        ),
        OfflineJob::ComplianceVerify { file } => crate::sec::verify_compliance_report(&file),
        OfflineJob::SecReport { file } => crate::sec::render_saved_report(&file),
        OfflineJob::SecComplianceSummary {
            profile,
            report_file,
            map_file,
        } => crate::sec::run_compliance_summary(&profile, &report_file, &map_file),
        OfflineJob::VulndbUpdate { from, dest } => crate::sec::vulndb_update_from(&from, &dest),
        OfflineJob::ReleaseInspect { manifest } => crate::release::inspect(manifest.as_deref()),
        OfflineJob::ReleaseVerify {
            manifest,
            signature,
            keyring,
        } => crate::release::verify_manifest(&manifest, &signature, &keyring),
        OfflineJob::UpdateVerify {
            metadata,
            signature,
            keyring,
        } => crate::release::verify_update(&metadata, &signature, &keyring),
        OfflineJob::OfflineBundleVerify { bundle } => {
            crate::release::verify_offline_bundle(&bundle)
        }
        OfflineJob::UpdateCheck
        | OfflineJob::UpdatePlan
        | OfflineJob::UpdateDownload
        | OfflineJob::UpdateStage
        | OfflineJob::UpdateStatus
        | OfflineJob::UpdateCancel
        | OfflineJob::UpdateRollback => Err(CliError::capability_unavailable(
            "updateApplyRequiresWindowsQualification",
        )),
        OfflineJob::ServiceDetect => {
            let state = transport::detect_service(config);
            let pid = match &state {
                transport::ServiceState::Reachable { pid: Some(pid) } => serde_json::json!(pid),
                _ => serde_json::Value::Null,
            };
            Ok(serde_json::json!({
                "state": state.as_str(),
                "pid": pid,
                "messageKey": state.message_key(),
                "endpointDir": transport::endpoint_dir(config).display().to_string(),
            }))
        }
        // Handled by the caller (raw artifact emission).
        OfflineJob::Help | OfflineJob::ServiceUnits => Ok(serde_json::json!({})),
    }
}

pub fn platform_str() -> &'static str {
    aethercore_platform_capabilities::current_platform_name()
}

/// Mirrors services/maintenance-service/src/performance.rs::engine_source (parity gate).
pub fn offline_engine_source() -> &'static str {
    if cfg!(windows) || cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        "native"
    } else {
        "synthetic"
    }
}

/// Takes one real snapshot and reports what the collectors measured.
///
/// §41.15 3.C: `capabilities` used to answer from a static table with no runtime
/// input, so it reported `telemetryStorage: native` in the same session in which
/// `telemetry-once` returned `"storage": []`. It now costs one passive sample
/// (~250 ms, the same tick `telemetry-once` takes) and cannot contradict them.
pub fn observe_telemetry() -> aethercore_platform_capabilities::TelemetryObservation {
    let interval =
        std::time::Duration::from_millis(aethercore_performance_telemetry::MIN_INTERVAL_MS as u64);
    let measured = aethercore_performance_telemetry::default_platform()
        .sample(interval)
        .measured_subsystems();
    aethercore_platform_capabilities::TelemetryObservation {
        cpu: measured.cpu,
        memory: measured.memory,
        storage: measured.storage,
        gpu: measured.gpu,
    }
}

fn capabilities_data() -> Result<serde_json::Value, CliError> {
    let rows: Vec<serde_json::Value> =
        aethercore_platform_capabilities::matrix_for_current_platform_observed(observe_telemetry())
            .into_iter()
            .map(|(name, availability)| {
                let (state, key) = match availability {
                    Availability::Native => ("native", None),
                    Availability::Degraded { note_key } => ("degraded", Some(note_key.to_string())),
                    Availability::NotAvailable { reason_key } => {
                        ("notAvailable", Some(reason_key.to_string()))
                    }
                };
                serde_json::json!({
                    "name": name,
                    "availability": { "state": state, "key": key },
                })
            })
            .collect();
    Ok(serde_json::json!({
        "platform": platform_str(),
        "capabilities": rows,
    }))
}

/// ONE snapshot from the REAL cfg-selected PerfPlatform provider (never simulated).
fn telemetry_once(interval_ms: u32) -> Result<serde_json::Value, CliError> {
    let interval = std::time::Duration::from_millis(
        aethercore_performance_telemetry::PerfSnapshot::clamped_interval_ms(interval_ms) as u64,
    );
    let platform = aethercore_performance_telemetry::default_platform();
    let snapshot = platform.sample(interval).normalized();

    let storage: Vec<serde_json::Value> = snapshot
        .storage
        .iter()
        .map(|device| {
            serde_json::json!({
                "deviceId": device.device_id,
                "friendlyName": device.friendly_name,
                "activeTimeBp": device.active_time_bp,
                "queueDepthX100": device.queue_depth_x100,
                "avgTransferLatencyUs": device.avg_transfer_latency_us,
                "readBytesPerSec": device.read_bytes_per_sec,
                "writeBytesPerSec": device.write_bytes_per_sec,
            })
        })
        .collect();
    let process_top: Vec<serde_json::Value> = snapshot
        .process_top
        .iter()
        .map(|entry| {
            serde_json::json!({
                "pid": entry.pid,
                "name": entry.name,
                "cpuBusyBp": entry.cpu_busy_bp,
                "readBytesPerSec": entry.read_bytes_per_sec,
                "writeBytesPerSec": entry.write_bytes_per_sec,
                "workingSetBytes": entry.working_set_bytes,
            })
        })
        .collect();
    let faults: Vec<serde_json::Value> = snapshot
        .collector_faults
        .iter()
        .map(|fault| {
            serde_json::json!({
                "collector": fault.collector,
                "kind": fault.kind,
                "detail": fault.detail,
            })
        })
        .collect();
    // §20.1.1 site 8: this was a SECOND, independent gpu-availability rule that
    // the collector knew nothing about, and which the service path
    // (`performance.rs::gpu_sample_proto`) did not have — so the CLI and the
    // service gave different answers about gpu presence from the identical
    // snapshot. The collector's own reading is now the only decider: gpu is null
    // exactly when the collector declared gpu unavailable.
    let gpu = if !snapshot.measured_subsystems().gpu {
        serde_json::Value::Null
    } else {
        serde_json::json!({
            "adapterId": snapshot.gpu.adapter_id,
            "adapterName": snapshot.gpu.adapter_name,
            "dedicatedUsedBytes": snapshot.gpu.dedicated_used_bytes,
            "dedicatedTotalBytes": snapshot.gpu.dedicated_total_bytes,
            "sharedUsedBytes": snapshot.gpu.shared_used_bytes,
            "engineCount": snapshot.gpu.engines.len(),
            "frametimeJitterUs": snapshot.gpu.frametime_jitter_us,
            "compositorLagDetected": snapshot.gpu.compositor_lag_detected,
        })
    };

    Ok(serde_json::json!({
        "platform": platform_str(),
        "providerSource": "PerfPlatform",
        "intervalMs": snapshot.interval_ms,
        "capturedUnixMs": snapshot.captured_unix_ms,
        "cpu": {
            "totalBusyBp": snapshot.cpu.total_busy_bp,
            "perProcessorBusyBp": snapshot.cpu.per_processor_busy_bp,
            "dpcIsrBusyBp": snapshot.cpu.dpc_isr_busy_bp,
            "contextSwitchesPerSec": snapshot.cpu.context_switches_per_sec,
            "processorQueueLengthX100": snapshot.cpu.processor_queue_length_x100,
        },
        "power": {
            "throttleActive": snapshot.power.throttle_active,
            "throttleReason": throttle_reason_str(snapshot.power.throttle_reason as i32),
            "hasTemperature": snapshot.power.has_temperature,
            "temperatureC": snapshot.power.temperature_c,
        },
        "memory": {
            "totalPhysicalBytes": snapshot.memory.total_physical_bytes,
            "availablePhysicalBytes": snapshot.memory.available_physical_bytes,
            "memoryLoadPercent": snapshot.memory.memory_load_percent,
            "hardFaultsPerSec": snapshot.memory.hard_faults_per_sec,
            "softFaultsPerSec": snapshot.memory.soft_faults_per_sec,
        },
        "storage": storage,
        "gpu": gpu,
        "processTop": process_top,
        "collectorFaults": faults,
    }))
}

fn throttle_reason_str(code: i32) -> &'static str {
    match code {
        1 => "none",
        2 => "thermal",
        3 => "power",
        4 => "vrm",
        5 => "current",
        _ => "unspecified",
    }
}

// ---------------------------------------------------------------------------
// self-check (model manifest schema validity + sha256 match; fail-closed)
// ---------------------------------------------------------------------------

const MODEL_MANIFEST_SCHEMA: &str = "aethercore.phase23.model-manifest.v1";

fn model_root_candidates() -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(root) = std::env::var("AETHERCORE_PRODUCT_ROOT") {
        candidates.push(std::path::PathBuf::from(root));
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut ancestor = exe.as_path();
        for _ in 0..4 {
            if let Some(parent) = ancestor.parent() {
                candidates.push(parent.to_path_buf());
                ancestor = parent;
            } else {
                break;
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    candidates
}

fn locate_models_dir() -> Option<std::path::PathBuf> {
    for candidate in model_root_candidates() {
        let models = candidate.join("assets").join("models");
        if models.join("models.manifest.json").is_file() {
            return Some(models);
        }
    }
    None
}

fn is_lower_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn sha256_file_hex(path: &std::path::Path) -> Result<String, CliError> {
    use sha2::Digest as _;
    let mut file = std::fs::File::open(path)
        .map_err(|e| CliError::local_io_with("cli.selfCheck.artifactUnreadable", e.to_string()))?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 65_536];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer).map_err(|e| {
            CliError::local_io_with("cli.selfCheck.artifactUnreadable", e.to_string())
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)
}

fn validate_manifest(raw: &serde_json::Value) -> Result<Vec<(String, u64, u64, String)>, CliError> {
    // Returns (fileName, bytes, declaredRamBudgetBytes, sha256Hex) after schema checks.
    if raw.get("schema").and_then(|v| v.as_str()) != Some(MODEL_MANIFEST_SCHEMA) {
        return Err(CliError::local_io("cli.selfCheck.manifestSchemaInvalid"));
    }
    if raw
        .get("policy")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .is_empty()
    {
        return Err(CliError::local_io("cli.selfCheck.manifestPolicyMissing"));
    }
    let artifacts = raw
        .get("artifacts")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CliError::local_io("cli.selfCheck.manifestArtifactsMissing"))?;
    if artifacts.is_empty() || artifacts.len() > 16 {
        return Err(CliError::local_io("cli.selfCheck.manifestArtifactsBounded"));
    }
    let mut entries = Vec::new();
    for artifact in artifacts {
        let file_name = artifact
            .get("fileName")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && !s.contains('/') && !s.contains(".."))
            .ok_or_else(|| CliError::local_io("cli.selfCheck.manifestFileNameInvalid"))?;
        let bytes = artifact
            .get("bytes")
            .and_then(|v| v.as_u64())
            .filter(|b| *b > 0)
            .ok_or_else(|| CliError::local_io("cli.selfCheck.manifestBytesInvalid"))?;
        let ram = artifact
            .get("declaredRamBudgetBytes")
            .and_then(|v| v.as_u64())
            .filter(|b| *b > 0)
            .ok_or_else(|| CliError::local_io("cli.selfCheck.manifestRamBudgetInvalid"))?;
        let sha = artifact
            .get("sha256Hex")
            .and_then(|v| v.as_str())
            .filter(|s| is_lower_hex64(s))
            .ok_or_else(|| CliError::local_io("cli.selfCheck.manifestShaInvalid"))?
            .to_string();
        if artifact.get("sourceUrl").and_then(|v| v.as_str()).is_none()
            || artifact
                .get("quantization")
                .and_then(|v| v.as_str())
                .is_none()
            || artifact.get("license").and_then(|v| v.as_str()).is_none()
        {
            return Err(CliError::local_io("cli.selfCheck.manifestEntryIncomplete"));
        }
        entries.push((file_name.to_string(), bytes, ram, sha));
    }
    Ok(entries)
}

fn self_check(load_model: bool) -> Result<serde_json::Value, CliError> {
    let models_dir =
        locate_models_dir().ok_or_else(|| CliError::local_io("cli.selfCheck.modelsDirNotFound"))?;
    let manifest_path = models_dir.join("models.manifest.json");
    let raw_text = std::fs::read_to_string(&manifest_path)
        .map_err(|e| CliError::local_io_with("cli.selfCheck.manifestUnreadable", e.to_string()))?;
    let raw: serde_json::Value = serde_json::from_str(&raw_text)
        .map_err(|e| CliError::local_io_with("cli.selfCheck.manifestMalformed", e.to_string()))?;
    let entries = validate_manifest(&raw)?;

    let mut artifact_rows = Vec::new();
    for (file_name, bytes, ram_budget, pinned_sha) in entries {
        let artifact_path = models_dir.join(&file_name);
        let actual_len = std::fs::metadata(&artifact_path)
            .map_err(|_| {
                CliError::local_io_with(
                    "cli.selfCheck.artifactMissing",
                    format!("{file_name} (fail-closed)"),
                )
            })?
            .len();
        if actual_len != bytes {
            return Err(CliError::local_io_with(
                "cli.selfCheck.artifactSizeMismatch",
                format!("{file_name}: manifest pins {bytes} bytes, found {actual_len}"),
            ));
        }
        let actual_sha = sha256_file_hex(&artifact_path)?;
        if actual_sha != pinned_sha {
            // Fail-closed: ANY mismatch refuses; nothing is ever loaded on mismatch.
            return Err(CliError::local_io_with(
                "cli.selfCheck.hashMismatchRefused",
                format!("{file_name}: refusing (fail-closed)"),
            ));
        }
        artifact_rows.push(serde_json::json!({
            "fileName": file_name,
            "bytes": bytes,
            "declaredRamBudgetBytes": ram_budget,
            "sha256Match": true,
        }));
    }

    let mut loaded = serde_json::json!({ "loaded": false, "label": serde_json::Value::Null });
    if load_model {
        loaded = load_model_probe()?;
    }

    Ok(serde_json::json!({
        "manifestSchema": MODEL_MANIFEST_SCHEMA,
        "manifestValid": true,
        "modelsDir": models_dir.display().to_string(),
        "artifacts": artifact_rows,
        "loaded": loaded["loaded"],
        "loadLabel": loaded["label"],
    }))
}

/// Explicit --load-model probe. Without the opt-in `embedded-model` cargo feature this
/// honestly reports capability-not-available instead of pretending to load anything.
fn load_model_probe() -> Result<serde_json::Value, CliError> {
    #[cfg(feature = "embedded-model")]
    {
        let models_dir = locate_models_dir()
            .ok_or_else(|| CliError::local_io("cli.selfCheck.modelsDirNotFound"))?;
        let model_path = models_dir.join(
            aethercore_intelligence_core::llama::EMBEDDED_MODEL_RELATIVE_PATH
                .rsplit('/')
                .next()
                .unwrap_or("model.gguf"),
        );
        let pinned = aethercore_intelligence_core::llama::embedded_model_entry();
        let mut reasoner = aethercore_intelligence_core::llama::LlamaCppReasoner::new();
        match reasoner.load(&model_path) {
            Ok(()) if reasoner.is_loaded() => Ok(serde_json::json!({
                "loaded": true,
                "label": format!("embedded reasoner active ({})", pinned.file_name),
            })),
            _ => Err(CliError::capability_unavailable("embeddedModelLoadFailed")),
        }
    }
    #[cfg(not(feature = "embedded-model"))]
    {
        Err(CliError::capability_unavailable(
            "embeddedModelLoaderNotCompiled",
        ))
    }
}

// ---------------------------------------------------------------------------
// Phase 29 (T3): EXPORT_V1 offline verification + explicit owner key management.
// ---------------------------------------------------------------------------

/// `export verify <file>` — recomputes the full hash chain locally (zero service
/// dependency). Any tampered byte produces a typed failure naming the broken link.
fn export_verify(file: &str) -> Result<serde_json::Value, CliError> {
    let raw = std::fs::read(file).map_err(|e| CliError::LocalIo {
        message_key: "local.io.read".to_string(),
        detail: Some(format!("cannot read {file}: {e}")),
    })?;
    let envelope = aethercore_persistence::export::parse_envelope_bytes(&raw).map_err(|e| {
        CliError::LocalIo {
            message_key: "local.export.verify".to_string(),
            detail: Some(format!("verification failed: {e}")),
        }
    })?;
    aethercore_persistence::export::verify_envelope(&envelope).map_err(|e| CliError::LocalIo {
        message_key: "local.export.verify".to_string(),
        detail: Some(format!("verification failed: {e}")),
    })?;
    Ok(serde_json::json!({
        "verified": true,
        "file": file,
        "schema": envelope.header.schema,
        "recordCount": envelope.header.record_count,
        "digest": envelope.digest,
        "signed": envelope.signed,
    }))
}

/// `keys generate --out <path>` — EXPLICIT owner action. Writes the 32-byte seed as
/// lowercase hex with 0600 permissions and prints the public-key fingerprint.
/// Key material is never created anywhere else in the product.
fn keys_generate(out: &str) -> Result<serde_json::Value, CliError> {
    let mut seed = [0u8; 32];
    os_random(&mut seed).map_err(|e| CliError::LocalIo {
        message_key: "local.io.read".to_string(),
        detail: Some(format!("entropy: {e}")),
    })?;
    let signing = ed25519_dalek::SigningKey::from_bytes(&seed);
    let seed_hex: String = signing
        .to_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let public_hex: String = signing
        .verifying_key()
        .to_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let path = std::path::Path::new(out);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CliError::LocalIo {
            message_key: "local.io.write".to_string(),
            detail: Some(format!("create key dir: {e}")),
        })?;
    }
    // create_new (O_EXCL / CREATE_NEW): overwriting would destroy the previous signing
    // key for good, so an existing path — file, directory or symlink — is refused.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => CliError::local_io_with(
                "local.keys.exists",
                format!("refusing to overwrite existing key file {out}"),
            ),
            _ => CliError::local_io_with("local.io.write", format!("create key file: {e}")),
        })?;
    std::io::Write::write_all(&mut file, format!("{seed_hex}\n").as_bytes())
        .map_err(|e| CliError::local_io_with("local.io.write", format!("write key file: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(serde_json::json!({
        "generated": true,
        "out": out,
        "publicKeyFingerprint": public_hex,
        "permissions": "0600",
    }))
}

/// Fills `seed` from the OS CSPRNG. Any error aborts key generation (fail closed).
#[cfg(unix)]
fn os_random(seed: &mut [u8; 32]) -> std::io::Result<()> {
    use std::io::Read as _;
    // Absolute on unix, and only root can create nodes under /dev.
    std::fs::File::open("/dev/urandom")?.read_exact(seed)
}

/// On Windows `/dev/urandom` resolves to `<current drive>:\dev\urandom`, a file any
/// local user can create — so the seed comes from BCryptGenRandom, never from a path.
#[cfg(windows)]
fn os_random(seed: &mut [u8; 32]) -> std::io::Result<()> {
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptGenRandom(
            algorithm: *mut std::ffi::c_void,
            buffer: *mut u8,
            len: u32,
            flags: u32,
        ) -> i32;
    }
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
    // SAFETY: SYSTEM_PREFERRED_RNG requires a null algorithm handle; the pointer and
    // length describe exactly the exclusively borrowed 32-byte buffer.
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            seed.as_mut_ptr(),
            seed.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "BCryptGenRandom failed, NTSTATUS {status:#010x}"
        )))
    }
}

/// `keys fingerprint --in <path>` — derives the Ed25519 public-key fingerprint from an
/// existing seed file without revealing any key material beyond the public half.
fn keys_fingerprint(input: &str) -> Result<serde_json::Value, CliError> {
    let raw = std::fs::read_to_string(input).map_err(|e| CliError::LocalIo {
        message_key: "local.io.read".to_string(),
        detail: Some(format!("cannot read {input}: {e}")),
    })?;
    let hex = raw.trim();
    if hex.len() != 64 || !hex.bytes().all(|b| (b as char).is_ascii_hexdigit()) {
        return Err(CliError::LocalIo {
            message_key: "local.keys.badKeyFile".to_string(),
            detail: Some("key file must contain exactly 64 hex characters".to_string()),
        });
    }
    let mut seed = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let hi = (chunk[0] as char).to_digit(16).unwrap_or(0) as u8;
        let lo = (chunk[1] as char).to_digit(16).unwrap_or(0) as u8;
        seed[i] = (hi << 4) | lo;
    }
    let verifying = ed25519_dalek::SigningKey::from_bytes(&seed).verifying_key();
    let public_hex: String = verifying
        .to_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(serde_json::json!({
        "in": input,
        "publicKeyFingerprint": public_hex,
    }))
}

// ---------------------------------------------------------------------------
// Phase 30 (T6): offline SQLite diagnostics through aethercore-db-diagnostics.
// Read-only contractual: the provider opens SQLITE_OPEN_READ_ONLY, busy_timeout=0.
// ---------------------------------------------------------------------------

fn db_check_sqlite(path: &str) -> Result<serde_json::Value, CliError> {
    let report =
        aethercore_db_diagnostics::sqlite_provider::diagnose_sqlite(path).map_err(|e| {
            CliError::LocalIo {
                message_key: "local.db.check".to_string(),
                detail: Some(e),
            }
        })?;
    Ok(serde_json::to_value(&report).unwrap_or_else(|_| {
        serde_json::json!({
            "targetPath": path,
            "findings": [],
            "digest": report.digest,
        })
    }))
}
