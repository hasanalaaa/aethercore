//! P75 Windows smoke: One-Click Care end to end against the INSTALLED service, over the
//! same named pipe the desktop uses. `windows-installer.yml` (bundle-log-acl-probe) runs
//! it after installing the real bundle and gates on its exit code and `SMOKE:` lines.
//!
//! Flow: cleanup scan → a plan from the candidates the cleaner selects by default → the
//! care plan preview → one approval (its digest must be the preview's) → one run (every
//! automatic step must reach its domain; none may be refused for authorization) → a second
//! run without a new approval must not run (the approval was used up) → a fresh session
//! must retain the exact run on Timeline. `--verify-run-id <care-run-id>` is read-only: a fresh
//! session queries Timeline without starting a scan, granting consent, or running Care.

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Run,
    VerifyRun(String),
}

fn parse_mode(args: &[String]) -> Result<Mode, String> {
    match args {
        [] => Ok(Mode::Run),
        [flag, run_id] if flag == "--verify-run-id" => {
            // The service names a run `care-<UTC milliseconds>` (router/care.rs); an i64 has
            // at most 19 digits.
            let millis = run_id.strip_prefix("care-").unwrap_or_default();
            if !(1..=19).contains(&millis.len()) || !millis.bytes().all(|b| b.is_ascii_digit()) {
                return Err(
                    "--verify-run-id requires the service's care-<milliseconds> run id".into(),
                );
            }
            Ok(Mode::VerifyRun(run_id.into()))
        }
        _ => Err("usage: care_smoke [--verify-run-id <care-run-id>]".into()),
    }
}

#[cfg(any(windows, test))]
fn require_run_entry(
    page: &aethercore_contracts::v1::TimelineResponse,
    run_id: &str,
) -> Result<(), String> {
    let source = format!("care-run:{run_id}");
    if page.reload_required || !page.entries.iter().any(|entry| entry.source_id == source) {
        return Err(format!(
            "persisted {source} is not in the current owner's newest timeline page"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod argument_tests {
    use super::*;
    // The identity the installed service issues (router/care.rs), as observed on the owner PC.
    const RUN: &str = "care-1791560738005";
    #[test]
    fn native_mode_requires_the_service_run_identity_and_declares_read_only_verification() {
        assert_eq!(parse_mode(&[]).unwrap(), Mode::Run);
        assert_eq!(
            parse_mode(&["--verify-run-id".into(), RUN.into()]).unwrap(),
            Mode::VerifyRun(RUN.into())
        );
        for args in [
            vec!["--unknown"],
            vec!["--verify-run-id"],
            vec!["--verify-run-id", "care-"],
            vec!["--verify-run-id", "care-17915607380a5"],
            vec!["--verify-run-id", "CARE-1791560738005"],
            vec!["--verify-run-id", "care-123456789012345678901"],
            vec!["--verify-run-id", "ff0a7cd0-87cf-4152-9b98-326687fc634a"],
            vec!["--verify-run-id", RUN, "extra"],
        ] {
            assert!(parse_mode(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
        }
    }
    #[test]
    fn persistence_verification_requires_exact_loaded_care_source() {
        let mut page = aethercore_contracts::v1::TimelineResponse::default();
        assert!(require_run_entry(&page, RUN).is_err());
        page.entries.push(aethercore_contracts::v1::TimelineEntry {
            source_id: format!("execution:{RUN}"),
            ..Default::default()
        });
        assert!(require_run_entry(&page, RUN).is_err());
        page.entries[0].source_id = format!("care-run:{RUN}-other");
        assert!(require_run_entry(&page, RUN).is_err());
        page.entries[0].source_id = format!("care-run:{RUN}");
        assert!(require_run_entry(&page, RUN).is_ok());
        page.reload_required = true;
        assert!(require_run_entry(&page, RUN).is_err());
    }
}

#[cfg(not(windows))]
fn main() {
    match parse_mode(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(Mode::Run) => {
            eprintln!("care_smoke drives the Windows named pipe; it runs on Windows only")
        }
        Ok(Mode::VerifyRun(run_id)) => {
            eprintln!("care_smoke verifies persisted Care run {run_id} on Windows only")
        }
        Err(error) => {
            eprintln!("SMOKE: FAIL {error}");
            std::process::exit(1);
        }
    }
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Err(error) = parse_mode(&args).and_then(smoke::run) {
        println!("SMOKE: FAIL {error}");
        std::process::exit(1);
    }
    println!("SMOKE: PASS");
}

#[cfg(windows)]
mod smoke {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use aethercore_contracts::v1::{self, request::Payload as Req, response::Payload as Resp};
    use aethercore_ipc::SessionClient;

    const CALL: Duration = Duration::from_secs(60);

    fn call(client: &SessionClient, payload: Req) -> Result<Option<Resp>, String> {
        let request = v1::Request {
            header: Some(v1::RequestHeader {
                protocol_version: aethercore_contracts::PROTOCOL_VERSION,
                request_id: format!("care-smoke-{}", std::process::id()),
            }),
            payload: Some(payload),
        };
        let response = client
            .request(request, CALL)
            .map_err(|e| format!("transport: {e}"))?;
        if response.status_code != 0 {
            let error = response.error.unwrap_or_default();
            return Err(format!(
                "status {} {} {}",
                response.status_code, error.message_key, error.technical_detail
            ));
        }
        Ok(response.payload)
    }

    fn care(payload: Option<Resp>) -> Result<v1::CareRunStatus, String> {
        match payload {
            Some(Resp::CareStatus(v1::CareStatusResponse {
                status: Some(status),
            })) => Ok(status),
            other => Err(format!("expected a care status, got {other:?}")),
        }
    }

    fn cleanup(payload: Option<Resp>) -> Result<v1::CleanupSnapshot, String> {
        match payload {
            Some(Resp::CleanupSnapshot(v1::CleanupSnapshotResponse { snapshot: Some(s) })) => Ok(s),
            other => Err(format!("expected a cleanup snapshot, got {other:?}")),
        }
    }

    fn steps(status: &v1::CareRunStatus) -> String {
        status
            .steps
            .iter()
            .map(|s| {
                format!(
                    "{}:{}:{}:{}",
                    s.domain_kind, s.safety_level, s.outcome, s.failure_message_key
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn connect() -> Result<Arc<SessionClient>, String> {
        SessionClient::connect(
            "care-smoke",
            "0",
            0,
            Arc::new(|_| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
        )
        .map_err(|e| format!("connect: {e}"))
    }

    fn timeline(client: &SessionClient) -> Result<v1::TimelineResponse, String> {
        match call(
            client,
            Req::GetTimelinePage(v1::GetTimelinePageRequest {
                page_size: 100,
                before_sequence: 0,
                snapshot_cursor: None,
            }),
        )? {
            Some(Resp::TimelinePage(page)) => Ok(page),
            other => Err(format!("expected a timeline page, got {other:?}")),
        }
    }

    pub(super) fn run(mode: super::Mode) -> Result<(), String> {
        let client = connect()?;
        if let super::Mode::VerifyRun(run_id) = mode {
            super::require_run_entry(&timeline(&client)?, &run_id)?;
            println!("SMOKE: persisted-care-run={run_id}");
            return Ok(());
        }
        let scan = cleanup(call(&client, Req::StartCleanupScan(Default::default()))?)?;
        let deadline = Instant::now() + Duration::from_secs(180);
        let snapshot = loop {
            let snapshot = cleanup(call(&client, Req::GetCleanupSnapshot(Default::default()))?)?;
            if snapshot.scan_id == scan.scan_id && snapshot.state != "Scanning" {
                break snapshot;
            }
            if Instant::now() >= deadline {
                return Err(format!("cleanup scan still {} after 180 s", snapshot.state));
            }
            std::thread::sleep(Duration::from_millis(500));
        };
        let chosen: Vec<String> = snapshot
            .candidates
            .iter()
            .filter(|c| c.selected_by_default && !c.requires_explicit_confirmation)
            .map(|c| c.candidate_id.clone())
            .collect();
        println!(
            "SMOKE: cleanup scan state={} candidates={} chosen={}",
            snapshot.state,
            snapshot.candidates.len(),
            chosen.len()
        );
        if chosen.is_empty() {
            return Err(
                "the cleaner selected no candidate by default: nothing for care to run".into(),
            );
        }
        call(
            &client,
            Req::CreateCleanupPlan(v1::CreateCleanupPlanRequest {
                scan_id: snapshot.scan_id.clone(),
                inventory_epoch: snapshot.inventory_epoch,
                candidate_ids: chosen,
            }),
        )?;

        let shown = care(call(&client, Req::GetCareStatus(Default::default()))?)?;
        println!(
            "SMOKE: preview digest={} steps={}",
            shown.plan_digest_sha256,
            steps(&shown)
        );
        let auto = shown.steps.iter().filter(|s| s.safety_level <= 0).count();
        if auto == 0 {
            return Err("the care plan has no automatic step".into());
        }
        // The grant names the plan shown; the service approves nothing if it has changed.
        let granted = care(call(
            &client,
            Req::GrantCareSessionConsent(v1::GrantCareSessionConsentRequest {
                plan_digest_sha256: shown.plan_digest_sha256.clone(),
            }),
        )?)?;
        if granted.plan_digest_sha256 != shown.plan_digest_sha256 {
            return Err("the approval named a plan other than the one shown".into());
        }
        let started = Instant::now();
        let report = care(call(&client, Req::StartCareRun(Default::default()))?)?;
        println!(
            "SMOKE: run state={} summary={} ms={} steps={}",
            report.state,
            report.summary_key,
            started.elapsed().as_millis(),
            steps(&report)
        );
        if report.state != "Completed" {
            return Err(format!("the approved run ended {}", report.state));
        }
        for step in &report.steps {
            let ran = matches!(
                step.outcome.as_str(),
                "VerifiedByDomain" | "CompletedUnverified" | "Failed"
            );
            if step.safety_level <= 0 && !ran {
                return Err(format!(
                    "automatic step {} never reached its domain",
                    step.domain_plan_id
                ));
            }
            if step.safety_level > 0 && step.outcome != "Skipped" {
                return Err(format!(
                    "review-only step {} ran: {}",
                    step.domain_plan_id, step.outcome
                ));
            }
            if step.failure_message_key.contains("authorization") {
                return Err(format!(
                    "step {} refused for authorization",
                    step.domain_plan_id
                ));
            }
        }
        // P76 DBT-P76-006: the run and the cleanup it executed are on the newest timeline page
        // (0 = newest). On the owner's install the page stayed empty after a real run.
        let page = timeline(&client)?;
        super::require_run_entry(&page, &report.run_id)?;
        println!("SMOKE: care-run-id={}", report.run_id);
        let run_entry = format!("care-run:{}", report.run_id);
        let executions = page
            .entries
            .iter()
            .filter(|e| e.source_id.starts_with("execution:"))
            .count();
        println!(
            "SMOKE: timeline entries={} care run={} executions={executions}",
            page.entries.len(),
            page.entries.iter().any(|e| e.source_id == run_entry)
        );
        if !page.entries.iter().any(|e| e.source_id == run_entry) || executions == 0 {
            return Err("the care run is not on the timeline".into());
        }
        // A grant naming a plan that is not the one composed now approves nothing.
        match call(
            &client,
            Req::GrantCareSessionConsent(v1::GrantCareSessionConsentRequest {
                plan_digest_sha256: "0".repeat(64),
            }),
        ) {
            Err(error) if error.contains("care.error.planChanged") => {
                println!("SMOKE: a grant for a plan never shown was refused")
            }
            other => {
                return Err(format!(
                    "a grant for a plan never shown was not refused: {other:?}"
                ));
            }
        }
        let again = care(call(&client, Req::StartCareRun(Default::default()))?)?;
        println!("SMOKE: second run state={}", again.state);
        if again.state == "Completed" && !again.steps.is_empty() {
            return Err("a used-up approval ran a second time".into());
        }
        let previous_session = client.hello.session_id.clone();
        drop(client);
        let reconnected = connect()?;
        if reconnected.hello.session_id.is_empty()
            || reconnected.hello.session_id == previous_session
        {
            return Err("reconnect did not create a fresh IPC session".into());
        }
        super::require_run_entry(&timeline(&reconnected)?, &report.run_id)?;
        println!("SMOKE: reconnected-care-run={}", report.run_id);
        Ok(())
    }
}
