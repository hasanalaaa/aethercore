//! Phase 32 GD-4: LIVE host audit against THIS Mac's real configuration.
//! Prints a machine-readable summary + digest; run twice to prove stability.
//! Read-only by construction (the whole audit domain is).

use aethercore_security_audit as sec;

fn main() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/unknown".into());
    let targets = vec![
        // Real sshd_config on macOS (present when Remote Login is/was enabled).
        sec::model::AuditTarget::SshdConfig {
            path: "/etc/ssh/sshd_config".to_string(),
        },
        // Real sudoers (parse-only).
        sec::model::AuditTarget::Sudoers {
            path: "/etc/sudoers".to_string(),
        },
        // Real filesystem posture over the invoking user's home.
        sec::model::AuditTarget::FilesystemPaths {
            paths: vec![home.clone()],
        },
        // Firewall state via config-file presence only.
        sec::model::AuditTarget::FirewallState,
    ];
    let report = sec::run_audit(&targets);
    println!("GD4 platform={}", report.platform);
    println!("GD4 digest={}", report.digest);
    println!("GD4 lanes={}", report.lanes.len());
    for lane in &report.lanes {
        match &lane.status {
            sec::LaneStatus::Ok { .. } => {
                println!(
                    "GD4 lane={} status=ok findings={}",
                    lane.lane,
                    lane.findings.len()
                );
                for f in &lane.findings {
                    let locs: Vec<String> = f
                        .evidence
                        .iter()
                        .map(|e| e.source_location.clone())
                        .collect();
                    println!(
                        "GD4 finding id={} code={:?} sev={:?} locs={}",
                        f.id,
                        f.code,
                        f.severity,
                        locs.join(",")
                    );
                }
            }
            sec::LaneStatus::NotAvailable { reason } => {
                println!("GD4 lane={} status=notAvailable reason={reason}", lane.lane);
            }
        }
    }
}
