//! LOCAL-ONLY ENFORCEMENT (Phase 36).
//!
//! One test, one invariant: a crate that is supposed to work with no network must
//! not acquire a network-capable dependency — directly or transitively.
//!
//! Exactly three workspace crates are permitted a network stack, and each backs a
//! feature the user must explicitly start (see `docs/LOCAL_ONLY.md`). Everything
//! else — including `aethercore-intelligence-core`, which loads the embedded model
//! from disk — must resolve with no HTTP/TLS/socket crate anywhere in its graph.
//!
//! Fails the moment someone adds `reqwest` (or any peer below) to an offline crate.
//! No test framework, no new dependency: `cargo metadata` plus the `serde_json`
//! this crate already depends on.

use std::collections::{HashMap, HashSet};
use std::process::Command;

/// Crates that can open a socket. Transitive presence of ANY of these marks a
/// workspace crate network-capable.
const NETWORK_CAPABLE: &[&str] = &[
    "reqwest",
    "hyper",
    "hyper-util",
    "h2",
    "ureq",
    "curl",
    "isahc",
    "surf",
    "attohttpc",
    "tungstenite",
    "tokio-tungstenite",
    "quinn",
    "tonic",
    "axum",
    "warp",
    "actix-web",
    "native-tls",
    "openssl",
    "rustls",
    "trust-dns-resolver",
    "hickory-resolver",
];

/// The ONLY workspace crates allowed a network stack. Adding a name here is a
/// product decision, not a build fix — it must come with a `docs/LOCAL_ONLY.md`
/// entry saying which user action starts it and why it is off by default.
const NETWORK_ALLOWED: &[&str] = &[
    // `aetherctl update download` / desktop "Check for updates" — inert unless
    // update-trust.json exists AND sets enabled=true with >=1 channel. Ships
    // enabled=false, channels=[].
    "aethercore-update-download",
    // `driver download` for a driver the user selected from the local catalogue.
    "aethercore-driver-acquisition",
    // Links update-download so the desktop can offer the update action. Carries no
    // network call of its own.
    "aethercore-desktop",
];

#[test]
fn offline_crates_have_no_network_capable_dependency() {
    let workspace_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let out = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--offline"])
        .current_dir(workspace_root)
        .output()
        .expect("run cargo metadata");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).expect("parse metadata");

    let name_of: HashMap<&str, &str> = meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p["name"].as_str().unwrap()))
        .collect();

    // Normal + build edges only. Dev-dependencies cannot ship in the product.
    let edges: HashMap<&str, Vec<&str>> = meta["resolve"]["nodes"]
        .as_array()
        .expect("resolve nodes")
        .iter()
        .map(|n| {
            let deps = n["deps"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| {
                    d["dep_kinds"].as_array().unwrap().iter().any(|k| {
                        // kind is null for a normal dependency.
                        !matches!(k["kind"].as_str(), Some("dev"))
                    })
                })
                .map(|d| d["pkg"].as_str().unwrap())
                .collect();
            (n["id"].as_str().unwrap(), deps)
        })
        .collect();

    let banned: HashSet<&str> = NETWORK_CAPABLE.iter().copied().collect();
    let allowed: HashSet<&str> = NETWORK_ALLOWED.iter().copied().collect();

    let mut offenders: Vec<String> = Vec::new();
    for member in meta["workspace_members"].as_array().expect("members") {
        let member = member.as_str().unwrap();
        let member_name = name_of[member];
        if allowed.contains(member_name) {
            continue;
        }
        // Depth-first over the resolved graph.
        let mut seen: HashSet<&str> = HashSet::new();
        let mut stack = vec![member];
        let mut found: Vec<String> = Vec::new();
        while let Some(cur) = stack.pop() {
            for dep in edges.get(cur).into_iter().flatten() {
                if !seen.insert(dep) {
                    continue;
                }
                let dep_name = name_of[dep];
                if banned.contains(dep_name) {
                    found.push(format!("{dep_name} (pulled in by {})", name_of[cur]));
                }
                stack.push(dep);
            }
        }
        if !found.is_empty() {
            found.sort();
            found.dedup();
            offenders.push(format!("  {member_name}: {}", found.join(", ")));
        }
    }

    assert!(
        offenders.is_empty(),
        "LOCAL-ONLY VIOLATION — these crates must work with no network but now depend \
         on a network stack:\n{}\n\nThe product must run fully offline after install. \
         If this is deliberate, it is a product decision: add the crate to \
         NETWORK_ALLOWED in this test AND document in docs/LOCAL_ONLY.md which \
         explicit user action starts it and why it is off by default.",
        offenders.join("\n")
    );
}
