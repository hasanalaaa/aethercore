//! P75 Windows smoke: One-Click Care end to end against the INSTALLED service, over the
//! same named pipe the desktop uses. `windows-installer.yml` (bundle-log-acl-probe) runs
//! it after installing the real bundle and gates on its exit code and `SMOKE:` lines.
//!
//! Flow: cleanup scan → a plan from the candidates the cleaner selects by default → the
//! care plan preview → one approval (its digest must be the preview's) → one run (every
//! automatic step must reach its domain; none may be refused for authorization) → a second
//! run without a new approval must not run (the approval was used up).

#[cfg(not(windows))]
fn main() {
    eprintln!("care_smoke drives the Windows named pipe; it runs on Windows only");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    if let Err(error) = smoke::run() {
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

    pub fn run() -> Result<(), String> {
        let client = SessionClient::connect(
            "care-smoke",
            "0",
            0,
            Arc::new(|_| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
        )
        .map_err(|e| format!("connect: {e}"))?;

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
        Ok(())
    }
}
