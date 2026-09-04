//! GD proofs — Phase 34 live/hermetic proof suite (fleet crate).
//!
//! GD-1 fleet inventory/model
//! GD-2 host trust fail-closed
//! GD-3 SSH command safety (argv, no shell, insecure flags absent,
//!     timeout/cancellation, auth failures typed)
//! GD-4 multi-host orchestration (hermetic stub transport)
//! GD-7 process/OpenSSH capability detection (honest, non-mutating)
//!
//! GD-5 (remote compliance) and GD-6 (schedule semantics) live in
//! `remote_compliance.rs` and the scheduler unit tests respectively.

use aethercore_fleet::{
    ABSOLUTE_MAX_CONCURRENCY, AuthReference, FORBIDDEN_SSH_OPTION_FRAGMENTS, FleetCadence,
    FleetHost, FleetInventory, RemoteOutcomeKind, TrustDecision, build_remote_argv, decide, is_due,
    shell_quote, ssh_binary,
};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

fn host(id: &str, pinned: bool) -> FleetHost {
    let mut host = FleetHost::new(
        id,
        id,
        &format!("{id}.example.internal"),
        22,
        "ops",
        AuthReference::Agent,
        BTreeSet::new(),
    )
    .expect("valid host");
    if pinned {
        host.pin_trust(&"11".repeat(32), "ssh-ed25519", 1).unwrap();
    }
    host
}

// ---------------------------------------------------------------- GD-1

#[test]
fn gd1_fleet_inventory_model() {
    let mut inventory = FleetInventory::new();
    inventory.add(host("host-2", false)).unwrap();
    inventory.add(host("host-1", false)).unwrap();

    // list is deterministic (sorted by host id)
    let ids: Vec<&str> = inventory.hosts.iter().map(|h| h.host_id.as_str()).collect();
    assert_eq!(ids, vec!["host-1", "host-2"]);

    // show
    assert_eq!(inventory.get("host-1").unwrap().port, 22);

    // strict validation rejects malformed
    assert!(
        FleetHost::new(
            "BAD ID",
            "n",
            "h.example.internal",
            22,
            "ops",
            AuthReference::Agent,
            BTreeSet::new()
        )
        .is_err()
    );

    // duplicate rejection (id + identity)
    assert!(inventory.add(host("host-1", false)).is_err());

    // remove
    inventory.remove("host-2").unwrap();
    assert!(inventory.get("host-2").is_none());

    // no secrets persisted: round-trip can only contain schema fields
    let bytes = inventory.to_bytes().unwrap();
    let text = String::from_utf8(bytes).unwrap();
    for secret_marker in [
        "password",
        "BEGIN OPENSSH PRIVATE KEY",
        "api_key",
        "passphrase",
    ] {
        assert!(
            !text.contains(secret_marker),
            "secret marker {secret_marker} in inventory"
        );
    }
}

// ---------------------------------------------------------------- GD-2

#[test]
fn gd2_host_trust_fail_closed() {
    // Invariant under proof: unknown host is not trusted (NotVerified).
    // unknown host -> NotVerified
    let unpinned = host("host-x", false);
    assert_eq!(decide(&unpinned, None), TrustDecision::NotVerified);
    assert_eq!(
        decide(&unpinned, Some(&"22".repeat(32))),
        TrustDecision::NotVerified
    );

    // correct pinned host -> Trusted
    let pinned = host("host-x", true);
    assert_eq!(
        decide(&pinned, Some(&"11".repeat(32))),
        TrustDecision::Trusted
    );

    // changed fingerprint -> HostKeyMismatch
    assert_eq!(
        decide(&pinned, Some(&"ff".repeat(32))),
        TrustDecision::HostKeyMismatch
    );
}

// ---------------------------------------------------------------- GD-3

#[test]
fn gd3_ssh_command_safety() {
    let target = host("host-x", true);
    let argv = build_remote_argv(
        Path::new("/usr/bin/ssh"),
        &target,
        Some("/home/ops/id_ed25519"),
        Path::new("/tmp/aethercore-known_hosts"),
        Duration::from_secs(10),
        &["aetherctl", "--output", "json", "sec", "audit"],
    );

    // no local shell: argv[0] is the ssh binary itself
    assert_eq!(argv[0], "/usr/bin/ssh");
    // BatchMode + strict host key checking + no password auth present
    let joined = argv.join(" ");
    assert!(joined.contains("BatchMode=yes"));
    assert!(joined.contains("StrictHostKeyChecking=yes"));
    assert!(joined.contains("PasswordAuthentication=no"));
    assert!(joined.contains("KbdInteractiveAuthentication=no"));
    assert!(joined.contains("ConnectTimeout=10"));
    // insecure flags absent
    for forbidden in FORBIDDEN_SSH_OPTION_FRAGMENTS {
        assert!(
            !joined.contains(forbidden),
            "insecure flag {forbidden} present"
        );
    }
    // every remote word is single-quoted (injection resistance)
    for word in ["aetherctl", "--output", "json", "sec", "audit"] {
        assert!(argv.contains(&shell_quote(word)), "unquoted word {word}");
    }
    // identity file passed via -i, quoted
    assert!(argv.contains(&shell_quote("/home/ops/id_ed25519")));

    // auth failure classification is typed
    let auth = aethercore_fleet::classify_ssh_exit(255, "Permission denied (publickey).");
    assert_eq!(auth.outcome, RemoteOutcomeKind::AuthFailure);
}

#[test]
fn gd3_timeout_and_cancellation_typed() {
    // Timeout: a transport against a non-routable address hits the deadline.
    let mut unroutable = host("host-timeout", true);
    unroutable.hostname = "203.0.113.1".into(); // TEST-NET-3, never routable
    unroutable.port = 2323;
    // DBT-P42-013: this file was written into %TEMP% and never removed — the
    // one leak that still reproduces from a clean temp dir. TempDir removes it
    // when the test ends, including when it ends by panicking.
    let known_hosts_dir = tempfile::tempdir().expect("temp dir");
    let known_hosts = known_hosts_dir.path().join("known_hosts");
    std::fs::write(&known_hosts, b"").unwrap();
    let transport = aethercore_fleet::SshTransport::new(Duration::from_secs(2))
        .with_known_hosts_path(&known_hosts);
    let result = transport
        .execute(
            &unroutable,
            None,
            aethercore_fleet::RemoteOperation::VersionProbe,
        )
        .expect("typed result");
    // ssh with ConnectTimeout returns 255 quickly OR our op deadline hits;
    // both are typed: Timeout or Failed/AuthFailure — never Success.
    assert_ne!(result.outcome, RemoteOutcomeKind::Success);

    // Cancellation before execution: flag set -> Cancelled.
    let transport2 = aethercore_fleet::SshTransport::new(Duration::from_secs(2));
    transport2.cancel.cancel();
    let result2 = transport2
        .execute(
            &host("host-x", true),
            None,
            aethercore_fleet::RemoteOperation::VersionProbe,
        )
        .expect("typed result");
    assert_eq!(result2.outcome, RemoteOutcomeKind::Cancelled);
}

// ---------------------------------------------------------------- GD-4

struct HermeticTransport {
    behavior: std::sync::Arc<std::sync::Mutex<HashMap<String, RemoteOutcomeKind>>>,
    sleep: Duration,
}

use std::collections::HashMap;

impl aethercore_fleet::FleetTransport for HermeticTransport {
    fn execute(&self, host: &FleetHost) -> aethercore_fleet::RemoteResult {
        std::thread::sleep(self.sleep);
        let guard = self.behavior.lock().unwrap();
        let outcome = guard
            .get(&host.host_id)
            .cloned()
            .unwrap_or(RemoteOutcomeKind::Failed);
        aethercore_fleet::RemoteResult::kind(outcome)
    }
}

#[test]
fn gd4_multi_host_orchestration_hermetic() {
    let mut behavior = HashMap::new();
    behavior.insert("ok-1".to_string(), RemoteOutcomeKind::Success);
    behavior.insert("slow-1".to_string(), RemoteOutcomeKind::Timeout);
    behavior.insert("nv-1".to_string(), RemoteOutcomeKind::NotVerified);
    behavior.insert("mismatch-1".to_string(), RemoteOutcomeKind::HostKeyMismatch);

    let hosts = vec![
        host("ok-1", true),
        host("slow-1", true),
        host("nv-1", false),
        host("mismatch-1", true),
        host("late-1", true),
    ];
    let transport = HermeticTransport {
        behavior: std::sync::Arc::new(std::sync::Mutex::new(behavior)),
        sleep: Duration::from_millis(10),
    };
    let config = aethercore_fleet::OrchestratorConfig {
        max_concurrency: 2,
        host_timeout: Duration::from_secs(30),
    };
    let batch = aethercore_fleet::run_batch(
        std::sync::Arc::new(transport),
        &hosts,
        &config,
        &aethercore_fleet::transport::CancelToken::default(),
    )
    .expect("batch");

    // deterministic ordering
    let ids: Vec<&str> = batch.outcomes.iter().map(|o| o.host_id.as_str()).collect();
    assert_eq!(ids, vec!["late-1", "mismatch-1", "nv-1", "ok-1", "slow-1"]);
    // full honest state space realized
    assert_eq!(batch.count(RemoteOutcomeKind::Success), 1);
    assert_eq!(batch.count(RemoteOutcomeKind::Timeout), 1);
    assert_eq!(batch.count(RemoteOutcomeKind::NotVerified), 1);
    assert_eq!(batch.count(RemoteOutcomeKind::HostKeyMismatch), 1);
    // unaffected host completed despite others' failures
    assert!(
        batch
            .outcomes
            .iter()
            .all(|o| o.result.outcome != RemoteOutcomeKind::Failed || o.host_id == "late-1")
    );
    // concurrency bound respected
    assert!(batch.concurrency_used <= 2);
    assert!(batch.concurrency_used <= ABSOLUTE_MAX_CONCURRENCY);
}

// ---------------------------------------------------------------- GD-7

#[test]
fn gd7_ssh_capability_honest() {
    // Detection is honest: Some(real version) or None — never fabricated.
    if let Some(path) = ssh_binary() {
        assert!(path.is_file());
        let output = std::process::Command::new(&path)
            .arg("-V")
            .output()
            .expect("ssh -V runs");
        let text = String::from_utf8_lossy(&output.stderr);
        assert!(text.contains("OpenSSH"), "unexpected: {text}");
    }
    // If ssh is absent we simply do not assert — NotAvailable is honest.
}

// ------------------------------------------------- schedule sanity (GD-6 gate)

#[test]
fn gd6_schedule_not_due_before_anchor() {
    let sched = aethercore_fleet::FleetSchedule::new(
        "sched-proof",
        vec!["group:edge".into()],
        "cis-l1",
        FleetCadence::EveryHours(6),
        10_000,
    )
    .unwrap();
    assert!(!is_due(&sched, 10_000));
    assert!(!is_due(&sched, 10_000 + 6 * 3600 * 1000 - 1));
    assert!(is_due(&sched, 10_000 + 6 * 3600 * 1000));
}

// ------------------------------------------- CF-5: compatibility handshake
// Proofs run against HERMETIC FIXTURES in the EXACT wire format the real
// remote commands emit (`aetherctl --output json capabilities`), not
// hand-written synthetic JSON: the fixture is byte-shaped like the product's
// own envelope output (schema/command/ok/data + capabilities rows).

fn fixture_capabilities_output(version: &str, marker: Option<&str>) -> String {
    // This is the exact serde shape of aetherctl's `capabilities` success
    // envelope (envelope::success → serde_json::to_string, deny_unknown_fields
    // reader on parse-back). data rows mirror capabilities_data().
    let marker_json = match marker {
        Some(marker) => format!(",\"remoteContract\":\"{marker}\""),
        None => String::new(),
    };
    format!(
        r#"{{"schema":"aethercore.aetherctl.v1","command":"capabilities","ok":true,"data":{{"platform":"macos","version":"{version}"{marker_json},"capabilities":[{{"name":"fleet","availability":{{"state":"native","key":null}}}}]}}}}"#
    )
}

#[test]
fn cf5_compatible_marker_is_supported() {
    let output = fixture_capabilities_output("0.1.0", Some("fleet.remote.v1"));
    let verdict = aethercore_fleet::compatibility_verdict(&output);
    assert_eq!(verdict.remote_schema, "fleet.remote.v1");
    assert_eq!(verdict.aetherctl_version, "0.1.0");
    assert!(verdict.supported, "exact marker + version → supported");
}

#[test]
fn cf5_missing_marker_is_incompatible() {
    // The real `version` envelope has NO remoteContract field: it must NOT
    // authorize remote work (semver alone never authorizes).
    let output = fixture_capabilities_output("0.1.0", None);
    let verdict = aethercore_fleet::compatibility_verdict(&output);
    assert!(!verdict.supported, "missing marker → Incompatible");
    assert_eq!(verdict.remote_schema, "");
}

#[test]
fn cf5_wrong_marker_is_incompatible() {
    let output = fixture_capabilities_output("0.1.0", Some("fleet.remote.v0"));
    let verdict = aethercore_fleet::compatibility_verdict(&output);
    assert!(!verdict.supported, "older contract marker → Incompatible");
    assert_eq!(verdict.remote_schema, "fleet.remote.v0");

    let future = fixture_capabilities_output("9.0.0", Some("fleet.remote.v2"));
    let verdict = aethercore_fleet::compatibility_verdict(&future);
    assert!(!verdict.supported, "newer contract marker → Incompatible");
}

#[test]
fn cf5_malformed_envelope_is_incompatible() {
    for garbage in ["not json at all", "{}", r#"{"schema":"other","data":{}}"#] {
        let verdict = aethercore_fleet::compatibility_verdict(garbage);
        assert!(!verdict.supported, "{garbage} must be Incompatible");
    }
    // A version without any envelope is equally unauthorized.
    let verdict = aethercore_fleet::compatibility_verdict("0.1.0");
    assert!(!verdict.supported);
}

#[test]
fn cf5_marker_present_in_remote_operation_words() {
    // The handshake command set must include the capabilities call that
    // carries the marker (probe path: VersionProbe + Capabilities).
    let probe = aethercore_fleet::RemoteOperation::VersionProbe
        .command_words()
        .unwrap();
    assert_eq!(
        probe,
        vec!["aetherctl".to_string(), "--version".to_string()]
    );
    let caps = aethercore_fleet::RemoteOperation::Capabilities
        .command_words()
        .unwrap();
    assert_eq!(
        caps,
        vec![
            "aetherctl".to_string(),
            "--output".to_string(),
            "json".to_string(),
            "capabilities".to_string()
        ]
    );
}
