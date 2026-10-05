use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use aethercore_driver_backup::BackupEvidence;
use aethercore_driver_hub::{DiscoveryBackend, DriverHub, ScanState};
use aethercore_driver_install::{DriverInstallCoordinator, InstallPlatform};
use aethercore_operation_engine::{OperationEngine, PlanState};
use aethercore_operation_kernel::{
    MutationSupervisor, MutationWorkload, ReadBudgetManager, ReadWorkload,
};
use aethercore_persistence::Database;
use aethercore_restore_point::RestorePointEvidence;
use aethercore_windows_pnp::{DeviceRecord, DeviceStatus, DeviceVerification, InstalledDriver};
use aethercore_windows_update::{
    DiscoveryResult, DriverOffer, ExecutionStage, UpdateIdentity, VersionSource,
    WuaExecutionResult, WuaProgress, WuaUpdateResult,
};

const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn approve(engine: &OperationEngine, plan_id: &str) {
    let intent = engine
        .begin_consent_intent(plan_id, OWNER)
        .expect("consent intent");
    engine
        .approve_consent_intent(&intent.intent_id, OWNER, 4242)
        .expect("consent approval");
}

fn start_install(
    coordinator: &DriverInstallCoordinator,
    owner: &str,
    plan_id: &str,
) -> aethercore_driver_install::Result<aethercore_driver_install::InstallStatus> {
    let supervisor = MutationSupervisor::new();
    let lease = supervisor
        .try_acquire(MutationWorkload::DriverInstall, plan_id, owner)
        .expect("mutation lease");
    coordinator.start_with_lease(owner, plan_id, lease)
}

fn start_driver_scan(
    hub: &DriverHub,
    owner: &str,
) -> aethercore_driver_hub::Result<aethercore_driver_hub::DriverHubSnapshot> {
    let budget = ReadBudgetManager::new(4);
    let lease = budget
        .try_acquire(ReadWorkload::DriverDiscovery)
        .expect("read budget lease");
    hub.start_scan_with_lease(owner, lease, aethercore_driver_hub::SearchScope::Online)
}

#[derive(Clone)]
struct FakeDiscovery;
impl DiscoveryBackend for FakeDiscovery {
    fn inventory(&self) -> Result<Vec<DeviceRecord>, String> {
        Ok(vec![DeviceRecord {
            instance_id: "PCI\\VEN_FAKE&DEV_0001\\1".into(),
            display_name: "AetherCore Test Adapter".into(),
            description: "Test network adapter".into(),
            class_name: "Net".into(),
            class_guid: "{4d36e972-e325-11ce-bfc1-08002be10318}".into(),
            manufacturer: "AetherCore Labs".into(),
            enumerator: "PCI".into(),
            location: "PCI bus 1".into(),
            hardware_ids: vec!["PCI\\VEN_FAKE&DEV_0001".into()],
            compatible_ids: vec!["PCI\\CC_0200".into()],
            status: DeviceStatus::default(),
            driver: Some(driver("1.0.0.0", "oem1.inf")),
        }])
    }

    fn updates(&self) -> Result<DiscoveryResult, aethercore_driver_hub::DiscoveryFailure> {
        Ok(DiscoveryResult {
            offers: vec![DriverOffer {
                update_id: "11111111-2222-3333-4444-555555555555".into(),
                revision: 7,
                title: "AetherCore Labs - Net - 2.0.0.0".into(),
                hardware_id: "PCI\\VEN_FAKE&DEV_0001".into(),
                driver_class: "Net".into(),
                manufacturer: "AetherCore Labs".into(),
                model: "Test Adapter".into(),
                provider: "AetherCore Labs".into(),
                driver_date_iso: "2026-08-01".into(),
                device_problem_number: 0,
                device_status: 0,
                min_download_bytes: 10,
                max_download_bytes: 20,
                target_version: "2.0.0.0".into(),
                target_version_source: VersionSource::TitleHeuristic,
                support_url: String::new(),
            }],
            warnings: Vec::new(),
        })
    }
}

struct FakePlatform {
    mutated: AtomicBool,
    mutation_count: AtomicU32,
    events: Mutex<Vec<&'static str>>,
    reboot_required: bool,
    backup_fails: bool,
    restore_end_fails: bool,
    /// What PnP reports bound after the install, and its problem code; None is the offered
    /// driver, healthy.
    bound_after: Option<(InstalledDriver, u32)>,
    /// Windows hands back an existing restore point instead of a new one.
    restore_not_fresh: bool,
    /// System Restore is off: no point can be made.
    restore_unavailable: bool,
    /// A file in the export changes after it was sealed.
    drift_after_export: bool,
    /// The install thread stops right after the mutation, as a service that dies there would,
    /// until `released` is set.
    stall_after_install: bool,
    released: AtomicBool,
}

fn fake() -> FakePlatform {
    FakePlatform {
        mutated: AtomicBool::new(false),
        mutation_count: AtomicU32::new(0),
        events: Mutex::new(Vec::new()),
        reboot_required: false,
        backup_fails: false,
        restore_end_fails: false,
        bound_after: None,
        restore_not_fresh: false,
        restore_unavailable: false,
        drift_after_export: false,
        stall_after_install: false,
        released: AtomicBool::new(false),
    }
}

impl FakePlatform {
    fn push(&self, event: &'static str) {
        self.events.lock().expect("events").push(event);
    }
}

impl InstallPlatform for FakePlatform {
    fn boot_marker_ms(&self) -> Result<i64, String> {
        Ok(1_000)
    }

    fn verify_devices(&self, ids: &[String]) -> Result<Vec<DeviceVerification>, String> {
        self.push(if self.mutated.load(Ordering::SeqCst) {
            "verify-after"
        } else {
            "verify-before"
        });
        let mutated = self.mutated.load(Ordering::SeqCst);
        let (bound, problem) = match (&self.bound_after, mutated) {
            (Some((bound, problem)), true) => (bound.clone(), *problem),
            (None, true) => (driver("2.0.0.0", "oem2.inf"), 0),
            (_, false) => (driver("1.0.0.0", "oem1.inf"), 0),
        };
        Ok(ids
            .iter()
            .map(|id| DeviceVerification {
                instance_id: id.clone(),
                present: true,
                class_name: "Net".into(),
                hardware_ids: vec!["PCI\\VEN_FAKE&DEV_0001".into()],
                compatible_ids: vec!["PCI\\CC_0200".into()],
                status: DeviceStatus {
                    problem_code: problem,
                    has_problem: problem != 0,
                    ..DeviceStatus::default()
                },
                driver: Some(bound.clone()),
            })
            .collect())
    }

    fn begin_restore(&self, plan_id: &str) -> Result<RestorePointEvidence, String> {
        self.push("restore-begin");
        if self.restore_unavailable {
            return Err("System Restore is turned off".into());
        }
        Ok(RestorePointEvidence {
            sequence_number: 42,
            description: aethercore_restore_point::description_for_plan(plan_id),
            verified_fresh: !self.restore_not_fresh,
        })
    }

    fn end_restore(&self, _sequence: i64, _description: &str) -> Result<(), String> {
        self.push("restore-end");
        if self.restore_end_fails {
            return Err("injected restore END failure".into());
        }
        Ok(())
    }

    fn cancel_restore(&self, _sequence: i64, _description: &str) -> Result<(), String> {
        self.push("restore-cancel");
        Ok(())
    }

    fn backup_driver(&self, inf: &str, destination: &Path) -> Result<BackupEvidence, String> {
        self.push("backup");
        if self.backup_fails {
            return Err("injected OEM export failure".into());
        }
        std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
        std::fs::write(destination.join(inf), b"[Version]").map_err(|e| e.to_string())?;
        std::fs::write(destination.join("fake.sys"), b"driver").map_err(|e| e.to_string())?;
        let evidence =
            aethercore_driver_backup::seal_export(inf, destination).map_err(|e| e.to_string())?;
        if self.drift_after_export {
            std::fs::write(destination.join("fake.sys"), b"changed").map_err(|e| e.to_string())?;
        }
        Ok(evidence)
    }

    fn execute_wua(
        &self,
        identities: &[UpdateIdentity],
        progress: &mut dyn FnMut(WuaProgress),
        before_install: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<WuaExecutionResult, String> {
        self.push("wua-download");
        progress(WuaProgress {
            stage: ExecutionStage::Downloading,
            percent: 100,
            current_update_index: 0,
            current_update_percent: 100,
            bytes_downloaded: Some(20),
            bytes_total: Some(20),
        });
        before_install()?;
        self.push("wua-install");
        self.mutated.store(true, Ordering::SeqCst);
        self.mutation_count.fetch_add(1, Ordering::SeqCst);
        let stalled = Instant::now();
        while self.stall_after_install
            && !self.released.load(Ordering::SeqCst)
            && stalled.elapsed() < Duration::from_secs(10)
        {
            thread::sleep(Duration::from_millis(5));
        }
        progress(WuaProgress {
            stage: ExecutionStage::Installing,
            percent: 100,
            current_update_index: 0,
            current_update_percent: 100,
            bytes_downloaded: Some(0),
            bytes_total: Some(0),
        });
        Ok(WuaExecutionResult {
            result_code: "orcSucceeded".into(),
            hresult: 0,
            reboot_required: self.reboot_required,
            updates: identities
                .iter()
                .cloned()
                .map(|identity| WuaUpdateResult {
                    identity,
                    result_code: "orcSucceeded".into(),
                    hresult: 0,
                    reboot_required: self.reboot_required,
                })
                .collect(),
        })
    }
}

fn driver(version: &str, inf: &str) -> InstalledDriver {
    InstalledDriver {
        provider: "AetherCore Labs".into(),
        version: version.into(),
        inf_path: inf.into(),
        date: "2026-07-01".into(),
    }
}

/// Unique scratch root per invocation. A plain timestamp collides when cargo runs
/// this binary's tests in parallel on a coarse clock, so uniqueness is anchored by
/// a process-wide atomic counter (pid + counter + instant-nanos suffix, std only).
fn temp_root() -> std::path::PathBuf {
    use std::sync::atomic::AtomicU32;
    static SEQUENCE: AtomicU32 = AtomicU32::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::fs::canonicalize(std::env::temp_dir())
        .expect("real fixture parent")
        .join(format!(
            "aethercore-phase3-coordinator-{}-{sequence}-{nonce}",
            std::process::id()
        ))
}

fn ready_hub() -> Arc<DriverHub> {
    let hub = Arc::new(DriverHub::with_backend(Arc::new(FakeDiscovery)));
    start_driver_scan(&hub, OWNER).expect("start fake scan");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let snapshot = hub
            .snapshot_for_owner(OWNER)
            .expect("owned driver snapshot");
        if snapshot.state == ScanState::Ready {
            return hub;
        }
        assert!(
            snapshot.state != ScanState::Failed,
            "fake discovery failed: {}",
            snapshot.error_message
        );
        assert!(Instant::now() < deadline, "fake discovery timed out");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn protection_barrier_precedes_every_fake_mutation_and_completes() {
    let root = temp_root();
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("database"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let hub = ready_hub();
    let platform = Arc::new(FakePlatform {
        backup_fails: false,
        restore_end_fails: false,
        bound_after: None,
        ..fake()
    });
    let coordinator = DriverInstallCoordinator::with_platform(
        engine.clone(),
        hub.clone(),
        db.clone(),
        root.clone(),
        platform.clone(),
    );

    let snapshot = hub
        .snapshot_for_owner(OWNER)
        .expect("owned driver snapshot");
    let candidate_id = snapshot.devices[0].candidates[0].candidate_id.clone();
    let plan = coordinator
        .create_plan(
            OWNER,
            &snapshot.scan_id,
            snapshot.inventory_epoch,
            &[candidate_id],
        )
        .expect("plan");
    assert_eq!(plan.state, PlanState::AwaitingAuthorization);
    approve(&engine, &plan.id);
    start_install(&coordinator, OWNER, &plan.id).expect("start install");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = coordinator
            .status(OWNER, Some(&plan.id))
            .expect("status")
            .expect("execution");
        if status.plan_state == "Completed" {
            assert!(status.restore_point_verified);
            assert!(status.mutation_started);
            assert!(!status.recovery_required);
            assert!(status.items.iter().all(|item| item.verified));
            break;
        }
        assert!(
            status.plan_state != "Failed",
            "coordinator failed: {}",
            status.failure_message
        );
        assert!(
            Instant::now() < deadline,
            "coordinator timed out in {}",
            status.stage
        );
        thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(platform.mutation_count.load(Ordering::SeqCst), 1);
    let events = platform.events.lock().expect("events").clone();
    let pos = |name| {
        events
            .iter()
            .position(|event| *event == name)
            .unwrap_or_else(|| panic!("missing {name}: {events:?}"))
    };
    assert!(pos("wua-download") < pos("restore-begin"));
    assert!(pos("restore-begin") < pos("backup"));
    assert!(pos("backup") < pos("wua-install"));
    assert!(pos("wua-install") < pos("restore-end"));
    assert!(pos("restore-end") < pos("verify-after"));

    drop(coordinator);
    drop(engine);
    drop(db);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn backup_failure_cancels_protection_and_never_reaches_install() {
    let root = temp_root();
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("database"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let hub = ready_hub();
    let platform = Arc::new(FakePlatform {
        backup_fails: true,
        restore_end_fails: false,
        bound_after: None,
        ..fake()
    });
    let coordinator = DriverInstallCoordinator::with_platform(
        engine.clone(),
        hub.clone(),
        db.clone(),
        root.clone(),
        platform.clone(),
    );

    let snapshot = hub
        .snapshot_for_owner(OWNER)
        .expect("owned driver snapshot");
    let candidate_id = snapshot.devices[0].candidates[0].candidate_id.clone();
    let plan = coordinator
        .create_plan(
            OWNER,
            &snapshot.scan_id,
            snapshot.inventory_epoch,
            &[candidate_id],
        )
        .expect("plan");
    approve(&engine, &plan.id);
    start_install(&coordinator, OWNER, &plan.id).expect("start install");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = coordinator
            .status(OWNER, Some(&plan.id))
            .expect("status")
            .expect("execution");
        if status.plan_state == "Failed" {
            assert!(
                !status.mutation_started,
                "backup failure must occur before the mutation barrier"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "coordinator timed out in {}",
            status.stage
        );
        thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(platform.mutation_count.load(Ordering::SeqCst), 0);
    let events = platform.events.lock().expect("events").clone();
    assert!(events.contains(&"wua-download"));
    assert!(events.contains(&"restore-begin"));
    assert!(events.contains(&"backup"));
    assert!(events.contains(&"restore-cancel"));
    assert!(!events.contains(&"wua-install"));

    drop(coordinator);
    drop(engine);
    drop(db);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn restore_end_failure_after_mutation_requires_recovery_and_never_completes() {
    let root = temp_root();
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("database"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let hub = ready_hub();
    let platform = Arc::new(FakePlatform {
        backup_fails: false,
        restore_end_fails: true,
        bound_after: None,
        ..fake()
    });
    let coordinator = DriverInstallCoordinator::with_platform(
        engine.clone(),
        hub.clone(),
        db.clone(),
        root.clone(),
        platform.clone(),
    );

    let snapshot = hub
        .snapshot_for_owner(OWNER)
        .expect("owned driver snapshot");
    let candidate_id = snapshot.devices[0].candidates[0].candidate_id.clone();
    let plan = coordinator
        .create_plan(
            OWNER,
            &snapshot.scan_id,
            snapshot.inventory_epoch,
            &[candidate_id],
        )
        .expect("plan");
    approve(&engine, &plan.id);
    start_install(&coordinator, OWNER, &plan.id).expect("start install");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = coordinator
            .status(OWNER, Some(&plan.id))
            .expect("status")
            .expect("execution");
        if status.plan_state == "Failed" {
            assert!(
                status.mutation_started,
                "restore END failure occurs only after the install barrier"
            );
            assert!(
                status.recovery_required,
                "unclosed restore transaction must require recovery review"
            );
            assert_ne!(status.stage, "Completed");
            break;
        }
        assert_ne!(
            status.plan_state, "Completed",
            "restore END failure must never be reported as Completed"
        );
        assert!(
            Instant::now() < deadline,
            "coordinator timed out in {}",
            status.stage
        );
        thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(platform.mutation_count.load(Ordering::SeqCst), 1);
    let events = platform.events.lock().expect("events").clone();
    assert!(events.contains(&"wua-install"));
    assert!(events.contains(&"restore-end"));
    assert!(
        db.recovery_records(20)
            .expect("recovery history")
            .iter()
            .any(|r| r.plan_id == plan.id)
    );

    drop(coordinator);
    drop(engine);
    drop(db);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cross_user_start_cannot_fail_or_mutate_an_owned_plan() {
    const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let root = temp_root();
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("database"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let hub = ready_hub();
    let platform = Arc::new(FakePlatform {
        backup_fails: false,
        restore_end_fails: false,
        bound_after: None,
        ..fake()
    });
    let coordinator = DriverInstallCoordinator::with_platform(
        engine.clone(),
        hub.clone(),
        db.clone(),
        root.clone(),
        platform.clone(),
    );

    let snapshot = hub
        .snapshot_for_owner(OWNER)
        .expect("owned driver snapshot");
    let candidate_id = snapshot.devices[0].candidates[0].candidate_id.clone();
    let plan = coordinator
        .create_plan(
            OWNER,
            &snapshot.scan_id,
            snapshot.inventory_epoch,
            &[candidate_id],
        )
        .expect("plan");

    assert!(start_install(&coordinator, OTHER, &plan.id).is_err());
    let after = engine
        .get_plan_for_owner(&plan.id, OWNER)
        .expect("owner plan survives");
    assert_eq!(after.state, PlanState::AwaitingAuthorization);
    assert!(
        db.get_execution(&plan.id)
            .expect("execution query")
            .is_none()
    );
    assert_eq!(platform.mutation_count.load(Ordering::SeqCst), 0);

    drop(coordinator);
    drop(engine);
    drop(db);
    let _ = std::fs::remove_dir_all(root);
}

/// Runs one approved install whose device PnP reports `bound_after` afterwards, to the end.
fn install_ending_bound_to(
    bound_after: (InstalledDriver, u32),
) -> (String, String, bool, bool, u32) {
    let root = temp_root();
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("database"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let hub = ready_hub();
    let platform = Arc::new(FakePlatform {
        backup_fails: false,
        restore_end_fails: false,
        bound_after: Some(bound_after),
        ..fake()
    });
    let coordinator = DriverInstallCoordinator::with_platform(
        engine.clone(),
        hub.clone(),
        db.clone(),
        root.clone(),
        platform.clone(),
    );
    let snapshot = hub
        .snapshot_for_owner(OWNER)
        .expect("owned driver snapshot");
    let candidate_id = snapshot.devices[0].candidates[0].candidate_id.clone();
    let plan = coordinator
        .create_plan(
            OWNER,
            &snapshot.scan_id,
            snapshot.inventory_epoch,
            &[candidate_id],
        )
        .expect("plan");
    approve(&engine, &plan.id);
    start_install(&coordinator, OWNER, &plan.id).expect("start install");
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        let status = coordinator
            .status(OWNER, Some(&plan.id))
            .expect("status")
            .expect("execution");
        if status.plan_state == "Completed" || status.plan_state == "Failed" {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "coordinator timed out in {}",
            status.stage
        );
        thread::sleep(Duration::from_millis(10));
    };
    let outcome = (
        status.plan_state.clone(),
        status.items[0].detail.clone(),
        status.items[0].verified,
        status.recovery_required,
        platform.mutation_count.load(Ordering::SeqCst),
    );
    drop(coordinator);
    drop(engine);
    drop(db);
    let _ = std::fs::remove_dir_all(root);
    outcome
}

/// P84-03: Windows reporting success is not the proof; the driver bound afterwards is. Before
/// the install 1.0.0.0 (oem1.inf) is bound and 2.0.0.0 is offered. Red before: every one of
/// these read Completed and verified, because a present device without a problem code was
/// enough. None of them is rolled back automatically: one mutation, recovery review.
#[test]
fn an_install_is_verified_by_the_driver_bound_after_it_not_by_the_result_code() {
    for (label, bound) in [
        (
            "the old driver is still bound",
            (driver("1.0.0.0", "oem1.inf"), 0),
        ),
        (
            "an older driver than before",
            (driver("0.9.0.0", "oem3.inf"), 0),
        ),
        (
            "below the offered version",
            (driver("1.5.0.0", "oem2.inf"), 0),
        ),
        (
            "code 43 after the update",
            (driver("2.0.0.0", "oem2.inf"), 43),
        ),
    ] {
        let (state, detail, verified, recovery, mutations) = install_ending_bound_to(bound);
        assert_eq!(state, "Failed", "{label}: {detail}");
        assert!(!verified, "{label}");
        assert!(
            recovery,
            "{label}: a failed verification asks for recovery review"
        );
        assert_eq!(mutations, 1, "{label}: no automatic rollback");
    }
    // Compared as numbers: 13.0 is above the offered 2.0, where text order puts "13" first.
    let (state, detail, verified, _, _) =
        install_ending_bound_to((driver("13.0.0.0", "oem2.inf"), 0));
    assert_eq!(state, "Completed", "{detail}");
    assert!(verified);
}

/// Runs one approved install on `platform` to its end: the final plan state and whether it
/// crossed the mutation barrier.
fn install_on(platform: Arc<FakePlatform>) -> (String, bool, u32) {
    let root = temp_root();
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("database"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let hub = ready_hub();
    let coordinator = DriverInstallCoordinator::with_platform(
        engine.clone(),
        hub.clone(),
        db.clone(),
        root.clone(),
        platform.clone(),
    );
    let snapshot = hub
        .snapshot_for_owner(OWNER)
        .expect("owned driver snapshot");
    let candidate_id = snapshot.devices[0].candidates[0].candidate_id.clone();
    let plan = coordinator
        .create_plan(
            OWNER,
            &snapshot.scan_id,
            snapshot.inventory_epoch,
            &[candidate_id],
        )
        .expect("plan");
    approve(&engine, &plan.id);
    start_install(&coordinator, OWNER, &plan.id).expect("start install");
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        let status = coordinator
            .status(OWNER, Some(&plan.id))
            .expect("status")
            .expect("execution");
        if status.plan_state == "Completed" || status.plan_state == "Failed" {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "coordinator timed out in {}",
            status.stage
        );
        thread::sleep(Duration::from_millis(10));
    };
    let outcome = (
        status.plan_state.clone(),
        status.mutation_started,
        platform.mutation_count.load(Ordering::SeqCst),
    );
    drop(coordinator);
    drop(engine);
    drop(db);
    let _ = std::fs::remove_dir_all(root);
    outcome
}

/// P84-04: protection is proven, not assumed. A restore point Windows handed back from inside
/// its frequency window (not a new one), or an export that changed after it was sealed, stops
/// the install before the mutation barrier. Red before: the coordinator took both as given.
#[test]
fn protection_that_cannot_be_proven_stops_the_install_before_any_mutation() {
    for (label, platform) in [
        (
            "System Restore turned off",
            FakePlatform {
                restore_unavailable: true,
                ..fake()
            },
        ),
        (
            "a restore point that is not new",
            FakePlatform {
                restore_not_fresh: true,
                ..fake()
            },
        ),
        (
            "an export that changed after sealing",
            FakePlatform {
                drift_after_export: true,
                ..fake()
            },
        ),
    ] {
        let platform = Arc::new(platform);
        let (state, crossed, mutations) = install_on(platform.clone());
        assert_eq!(state, "Failed", "{label}");
        assert!(!crossed, "{label}: the barrier was crossed");
        assert_eq!(mutations, 0, "{label}");
        let events = platform.events.lock().expect("events").clone();
        // A point that was made is closed as cancelled; none is made when Restore is off.
        let made = !platform.restore_unavailable;
        assert_eq!(
            events.contains(&"restore-cancel"),
            made,
            "{label}: {events:?}"
        );
        assert!(!events.contains(&"wua-install"), "{label}: {events:?}");
    }
}

/// P84-04: a service that stops after the mutation barrier leaves a plan the next start marks
/// recovery-required, without replaying the install. The recovery path existed with no driver
/// test; this pins it.
#[test]
fn an_install_interrupted_after_the_barrier_is_recovery_required_and_not_replayed() {
    let root = temp_root();
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("database"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let hub = ready_hub();
    let platform = Arc::new(FakePlatform {
        stall_after_install: true,
        ..fake()
    });
    let coordinator = DriverInstallCoordinator::with_platform(
        engine.clone(),
        hub.clone(),
        db.clone(),
        root.clone(),
        platform.clone(),
    );
    let snapshot = hub
        .snapshot_for_owner(OWNER)
        .expect("owned driver snapshot");
    let candidate_id = snapshot.devices[0].candidates[0].candidate_id.clone();
    let plan = coordinator
        .create_plan(
            OWNER,
            &snapshot.scan_id,
            snapshot.inventory_epoch,
            &[candidate_id],
        )
        .expect("plan");
    approve(&engine, &plan.id);
    start_install(&coordinator, OWNER, &plan.id).expect("start install");
    let deadline = Instant::now() + Duration::from_secs(3);
    while platform.mutation_count.load(Ordering::SeqCst) == 0 {
        assert!(
            Instant::now() < deadline,
            "the install never crossed the barrier"
        );
        thread::sleep(Duration::from_millis(5));
    }

    // The service starts again, with the first install stuck where it died.
    let restarted = DriverInstallCoordinator::with_platform(
        engine.clone(),
        hub.clone(),
        db.clone(),
        root.clone(),
        Arc::new(fake()),
    );
    restarted.recover_incomplete().expect("recovery");
    let status = restarted
        .status(OWNER, Some(&plan.id))
        .expect("status")
        .expect("execution");
    assert_eq!(status.plan_state, "Failed");
    assert!(status.recovery_required);
    assert_eq!(status.stage, "RecoveryRequired");
    assert_eq!(
        platform.mutation_count.load(Ordering::SeqCst),
        1,
        "never replayed"
    );
    assert!(
        db.recovery_records(20)
            .expect("recovery history")
            .iter()
            .any(|r| r.plan_id == plan.id)
    );

    platform.released.store(true, Ordering::SeqCst);
    drop(restarted);
    drop(coordinator);
    drop(engine);
    drop(db);
    let _ = std::fs::remove_dir_all(root);
}
