//! DBT-P36-006: the filesystem lane's findings must rest on the permissions the host
//! really holds — POSIX mode bits on unix, the DACL on Windows — and every file under a
//! scan root must be examined, including the ones directly beneath it.
//!
//! The same three tests run on every platform; only the way a permission is planted
//! differs. On Windows they are the runner proof for DBT-P36-006: a default-ACL file must
//! raise nothing, and a file whose DACL grants Everyone write must raise SEC-FS-001 with
//! the SID and mask actually read.

use aethercore_security_audit::filesystem::audit_filesystem;
use aethercore_security_audit::model::SecFinding;
use std::path::Path;

fn scan_root(name: &str) -> tempfile::TempDir {
    // Not `/tmp` on unix: SANCTIONED_PREFIXES exempts it on Linux. Not the checkout on
    // Windows: a drive root's default ACL grants Authenticated Users modify on everything
    // created beneath it, so only the profile's own temp directory is private by default.
    let base = if cfg!(windows) {
        std::env::temp_dir()
    } else {
        std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
    };
    tempfile::Builder::new()
        .prefix(&format!("p36-006-{name}-"))
        .tempdir_in(base)
        .expect("tempdir")
}

fn scan(root: &Path) -> Vec<SecFinding> {
    audit_filesystem(&[root.to_str().expect("utf-8 temp path").to_string()]).expect("scan runs")
}

fn at<'a>(findings: &'a [SecFinding], id: &str, path: &Path) -> Option<&'a SecFinding> {
    let shown = path.display().to_string();
    findings
        .iter()
        .find(|f| f.id == id && f.evidence[0].source_location == shown)
}

#[cfg(windows)]
fn icacls_grant(path: &Path, grant: &str) {
    let status = std::process::Command::new("icacls")
        .arg(path)
        .args(["/grant", grant])
        .status()
        .expect("icacls runs");
    assert!(status.success(), "icacls {} /grant {grant}", path.display());
}

#[cfg(windows)]
fn current_user_sid() -> String {
    let output = std::process::Command::new("whoami")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .expect("whoami runs");
    assert!(output.status.success(), "whoami /user failed");
    let text = String::from_utf8(output.stdout).expect("UTF-8 whoami output");
    let sid = text
        .trim()
        .split(',')
        .next_back()
        .expect("SID column")
        .trim_matches('"');
    assert!(sid.starts_with("S-1-"), "unexpected whoami SID: {text}");
    sid.to_string()
}

#[cfg(unix)]
fn chmod(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
}

fn make_world_writable(path: &Path) {
    #[cfg(unix)]
    chmod(path, 0o666);
    // W is FILE_GENERIC_WRITE; *S-1-1-0 is Everyone by SID, so no localised name matters.
    #[cfg(windows)]
    icacls_grant(path, "*S-1-1-0:(W)");
}

/// Owner-only on unix. On Windows, remove inherited entries and grant only the
/// current user, SYSTEM and Administrators, so the fixture is truly private.
fn make_private(path: &Path, dir: bool) {
    #[cfg(unix)]
    chmod(path, if dir { 0o700 } else { 0o600 });
    #[cfg(windows)]
    {
        let _ = dir;
        let status = std::process::Command::new("icacls")
            .arg(path)
            .arg("/inheritance:r")
            .status()
            .expect("icacls runs");
        assert!(status.success(), "icacls /inheritance:r failed");
        let status = std::process::Command::new("icacls")
            .arg(path)
            .arg("/grant:r")
            .arg(format!("*{}:(F)", current_user_sid()))
            .args(["*S-1-5-18:(F)", "*S-1-5-32-544:(F)"])
            .status()
            .expect("icacls runs");
        assert!(status.success(), "icacls private grants failed");
    }
}

fn expose_to_others(path: &Path) {
    #[cfg(unix)]
    chmod(path, 0o640);
    #[cfg(windows)]
    icacls_grant(path, "*S-1-5-32-545:(R)");
}

#[test]
fn file_directly_under_the_scan_root_is_examined() {
    let root = scan_root("root-level");
    let loose = root.path().join("loose.txt");
    let nested_dir = root.path().join("sub");
    let nested = nested_dir.join("nested.txt");
    std::fs::create_dir(&nested_dir).unwrap();
    std::fs::write(&loose, b"x").unwrap();
    std::fs::write(&nested, b"x").unwrap();
    make_world_writable(&loose);
    make_world_writable(&nested);

    let findings = scan(root.path());

    let nested_hit = at(&findings, "SEC-FS-001", &nested);
    assert!(nested_hit.is_some(), "nested file missed: {findings:#?}");
    let loose_hit = at(&findings, "SEC-FS-001", &loose);
    assert!(
        loose_hit.is_some(),
        "a world-writable file directly under the scan root was never examined: {findings:#?}"
    );
    #[cfg(windows)]
    {
        let observed = &loose_hit.unwrap().evidence[0].observed;
        assert!(
            observed.contains("S-1-1-0") && observed.contains("mask=0x"),
            "evidence must name the ACE read, got {observed:?}"
        );
    }
}

#[test]
fn default_permissions_raise_no_world_writable_finding() {
    let root = scan_root("default");
    let sub = root.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    let plain = sub.join("plain.txt");
    std::fs::write(&plain, b"x").unwrap();
    // umask-independent on unix; on Windows the inherited ACL is left exactly as created.
    #[cfg(unix)]
    chmod(&plain, 0o644);

    let findings = scan(root.path());

    let ww: Vec<_> = findings.iter().filter(|f| f.id == "SEC-FS-001").collect();
    assert!(
        ww.is_empty(),
        "a default-permission file was reported world-writable: {ww:#?}"
    );
}

#[test]
fn ssh_findings_follow_the_real_permissions() {
    let root = scan_root("ssh");
    let ssh = root.path().join(".ssh");
    std::fs::create_dir(&ssh).unwrap();
    make_private(&ssh, true);
    let key = ssh.join("id_ed25519");
    std::fs::write(&key, b"-----BEGIN OPENSSH PRIVATE KEY-----\n").unwrap();
    make_private(&key, false);
    // A public key is public: ssh-keygen writes it 0644 and OpenSSH never checks it.
    let public = ssh.join("id_ed25519.pub");
    std::fs::write(&public, b"ssh-ed25519 AAAA test\n").unwrap();
    expose_to_others(&public);

    let quiet = scan(root.path());
    let ssh_hits: Vec<_> = quiet
        .iter()
        .filter(|f| f.id == "SEC-FS-003" || f.id == "SEC-FS-004")
        .collect();
    assert!(
        ssh_hits.is_empty(),
        "private .ssh material, or a public key, was reported over-permissive: {ssh_hits:#?}"
    );

    expose_to_others(&key);
    let loud = scan(root.path());
    let hit = at(&loud, "SEC-FS-004", &key);
    assert!(hit.is_some(), "exposed private key missed: {loud:#?}");
    #[cfg(windows)]
    {
        let observed = &hit.unwrap().evidence[0].observed;
        assert!(
            observed.contains("S-1-5-32-545"),
            "evidence must name the SID read, got {observed:?}"
        );
    }
}
