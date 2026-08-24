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
use aethercore_operation_kernel::{MutationSupervisor, MutationWorkload, ReadBudgetManager, ReadWorkload};
use aethercore_persistence::Database;
use aethercore_restore_point::RestorePointEvidence;
use aethercore_windows_pnp::{DeviceRecord, DeviceStatus, DeviceVerification, InstalledDriver};
use aethercore_windows_update::{
    DiscoveryResult, DriverOffer, ExecutionStage, UpdateIdentity, VersionSource,
    WuaExecutionResult, WuaProgress, WuaUpdateResult,
};

const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn approve(engine: &OperationEngine, plan_id: &str, digest: &str) {
    let intent = engine.begin_consent_intent(plan_id, OWNER).expect("consent intent");
    engine.approve_consent_intent(&intent.intent_id, OWNER, 4242).expect("consent approval");
}

fn start_install(coordinator: &DriverInstallCoordinator, owner: &str, plan_id: &str) -> aethercore_driver_install::Result<aethercore_driver_install::InstallStatus> {
    let supervisor = MutationSupervisor::new();
    let lease = supervisor.try_acquire(MutationWorkload::DriverInstall, plan_id, owner).expect("mutation lease");
    coordinator.start_with_lease(owner, plan_id, lease)
}

fn start_driver_scan(hub: &DriverHub, owner: &str) -> aethercore_driver_hub::Result<aethercore_driver_hub::DriverHubSnapshot> {
    let budget = ReadBudgetManager::new(4);
    let lease = budget.try_acquire(ReadWorkload::DriverDiscovery).expect("read budget lease");
    hub.start_scan_with_lease(owner, lease)
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
}

impl FakePlatform {
    fn push(&self, event: &'static str) {
        self.events.lock().expect("events").push(event);
    }
}

impl InstallPlatform for FakePlatform {
    fn boot_marker_ms(&self) -> Result<i64, String> { Ok(1_000) }

    fn verify_devices(&self, ids: &[String]) -> Result<Vec<DeviceVerification>, String> {
        self.push(if self.mutated.load(Ordering::SeqCst) { "verify-after" } else { "verify-before" });
        Ok(ids.iter().map(|id| DeviceVerification {
            instance_id: id.clone(),
            present: true,
            class_name: "Net".into(),
            hardware_ids: vec!["PCI\\VEN_FAKE&DEV_0001".into()],
            compatible_ids: vec!["PCI\\CC_0200".into()],
            status: DeviceStatus::default(),
            driver: Some(if self.mutated.load(Ordering::SeqCst) { driver("2.0.0.0", "oem2.inf") } else { driver("1.0.0.0", "oem1.inf") }),
        }).collect())
    }

    fn begin_restore(&self, plan_id: &str) -> Result<RestorePointEvidence, String> {
        self.push("restore-begin");
        Ok(RestorePointEvidence {
            sequence_number: 42,
            description: aethercore_restore_point::description_for_plan(plan_id),
            verified_fresh: true,
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
        Ok(BackupEvidence {
            source_inf: inf.into(),
            backup_directory: destination.display().to_string(),
            manifest_path: destination.join("aethercore-backup.json").display().to_string(),
            file_count: 3,
            total_bytes: 4096,
            not_applicable: false,
        })
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
            bytes_downloaded: 20,
            bytes_total: 20,
        });
        before_install()?;
        self.push("wua-install");
        self.mutated.store(true, Ordering::SeqCst);
        self.mutation_count.fetch_add(1, Ordering::SeqCst);
        progress(WuaProgress {
            stage: ExecutionStage::Installing,
            percent: 100,
            current_update_index: 0,
            current_update_percent: 100,
            bytes_downloaded: 0,
            bytes_total: 0,
        });
        Ok(WuaExecutionResult {
            result_code: "orcSucceeded".into(),
            hresult: 0,
            reboot_required: self.reboot_required,
            updates: identities.iter().cloned().map(|identity| WuaUpdateResult {
                identity,
                result_code: "orcSucceeded".into(),
                hresult: 0,
                reboot_required: self.reboot_required,
            }).collect(),
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

fn temp_root() -> std::path::PathBuf {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
    std::env::temp_dir().join(format!("aethercore-phase3-coordinator-{}-{nonce}", std::process::id()))
}

fn ready_hub() -> Arc<DriverHub> {
    let hub = Arc::new(DriverHub::with_backend(Arc::new(FakeDiscovery)));
    start_driver_scan(&hub, OWNER).expect("start fake scan");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let snapshot = hub.snapshot_for_owner(OWNER).expect("owned driver snapshot");
        if snapshot.state == ScanState::Ready { return hub; }
        assert!(snapshot.state != ScanState::Failed, "fake discovery failed: {}", snapshot.error_message);
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
        mutated: AtomicBool::new(false),
        mutation_count: AtomicU32::new(0),
        events: Mutex::new(Vec::new()),
        reboot_required: false,
        backup_fails: false,
        restore_end_fails: false,
    });
    let coordinator = DriverInstallCoordinator::with_platform(
        engine.clone(), hub.clone(), db.clone(), root.clone(), platform.clone(),
    );

    let snapshot = hub.snapshot_for_owner(OWNER).expect("owned driver snapshot");
    let candidate_id = snapshot.devices[0].candidates[0].candidate_id.clone();
    let plan = coordinator.create_plan(OWNER, &snapshot.scan_id, snapshot.inventory_epoch, &[candidate_id]).expect("plan");
    assert_eq!(plan.state, PlanState::AwaitingAuthorization);
    approve(&engine, &plan.id, &plan.digest);
    start_install(&coordinator, OWNER, &plan.id).expect("start install");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = coordinator.status(OWNER, Some(&plan.id)).expect("status").expect("execution");
        if status.plan_state == "Completed" {
            assert!(status.restore_point_verified);
            assert!(status.mutation_started);
            assert!(!status.recovery_required);
            assert!(status.items.iter().all(|item| item.verified));
            break;
        }
        assert!(status.plan_state != "Failed", "coordinator failed: {}", status.failure_message);
        assert!(Instant::now() < deadline, "coordinator timed out in {}", status.stage);
        thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(platform.mutation_count.load(Ordering::SeqCst), 1);
    let events = platform.events.lock().expect("events").clone();
    let pos = |name| events.iter().position(|event| *event == name).unwrap_or_else(|| panic!("missing {name}: {events:?}"));
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
        mutated: AtomicBool::new(false),
        mutation_count: AtomicU32::new(0),
        events: Mutex::new(Vec::new()),
        reboot_required: false,
        backup_fails: true,
        restore_end_fails: false,
    });
    let coordinator = DriverInstallCoordinator::with_platform(
        engine.clone(), hub.clone(), db.clone(), root.clone(), platform.clone(),
    );

    let snapshot = hub.snapshot_for_owner(OWNER).expect("owned driver snapshot");
    let candidate_id = snapshot.devices[0].candidates[0].candidate_id.clone();
    let plan = coordinator.create_plan(OWNER, &snapshot.scan_id, snapshot.inventory_epoch, &[candidate_id]).expect("plan");
    approve(&engine, &plan.id, &plan.digest);
    start_install(&coordinator, OWNER, &plan.id).expect("start install");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = coordinator.status(OWNER, Some(&plan.id)).expect("status").expect("execution");
        if status.plan_state == "Failed" {
            assert!(!status.mutation_started, "backup failure must occur before the mutation barrier");
            break;
        }
        assert!(Instant::now() < deadline, "coordinator timed out in {}", status.stage);
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
        mutated: AtomicBool::new(false),
        mutation_count: AtomicU32::new(0),
        events: Mutex::new(Vec::new()),
        reboot_required: false,
        backup_fails: false,
        restore_end_fails: true,
    });
    let coordinator = DriverInstallCoordinator::with_platform(
        engine.clone(), hub.clone(), db.clone(), root.clone(), platform.clone(),
    );

    let snapshot = hub.snapshot_for_owner(OWNER).expect("owned driver snapshot");
    let candidate_id = snapshot.devices[0].candidates[0].candidate_id.clone();
    let plan = coordinator.create_plan(OWNER, &snapshot.scan_id, snapshot.inventory_epoch, &[candidate_id]).expect("plan");
    approve(&engine, &plan.id, &plan.digest);
    start_install(&coordinator, OWNER, &plan.id).expect("start install");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = coordinator.status(OWNER, Some(&plan.id)).expect("status").expect("execution");
        if status.plan_state == "Failed" {
            assert!(status.mutation_started, "restore END failure occurs only after the install barrier");
            assert!(status.recovery_required, "unclosed restore transaction must require recovery review");
            assert_ne!(status.stage, "Completed");
            break;
        }
        assert_ne!(status.plan_state, "Completed", "restore END failure must never be reported as Completed");
        assert!(Instant::now() < deadline, "coordinator timed out in {}", status.stage);
        thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(platform.mutation_count.load(Ordering::SeqCst), 1);
    let events = platform.events.lock().expect("events").clone();
    assert!(events.contains(&"wua-install"));
    assert!(events.contains(&"restore-end"));
    assert!(db.recovery_records(20).expect("recovery history").iter().any(|r| r.plan_id == plan.id));

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
        mutated: AtomicBool::new(false),
        mutation_count: AtomicU32::new(0),
        events: Mutex::new(Vec::new()),
        reboot_required: false,
        backup_fails: false,
        restore_end_fails: false,
    });
    let coordinator = DriverInstallCoordinator::with_platform(
        engine.clone(), hub.clone(), db.clone(), root.clone(), platform.clone(),
    );

    let snapshot = hub.snapshot_for_owner(OWNER).expect("owned driver snapshot");
    let candidate_id = snapshot.devices[0].candidates[0].candidate_id.clone();
    let plan = coordinator
        .create_plan(OWNER, &snapshot.scan_id, snapshot.inventory_epoch, &[candidate_id])
        .expect("plan");

    assert!(start_install(&coordinator, OTHER, &plan.id).is_err());
    let after = engine.get_plan_for_owner(&plan.id, OWNER).expect("owner plan survives");
    assert_eq!(after.state, PlanState::AwaitingAuthorization);
    assert!(db.get_execution(&plan.id).expect("execution query").is_none());
    assert_eq!(platform.mutation_count.load(Ordering::SeqCst), 0);

    drop(coordinator);
    drop(engine);
    drop(db);
    let _ = std::fs::remove_dir_all(root);
}
