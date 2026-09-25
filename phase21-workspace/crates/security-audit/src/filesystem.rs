//! Phase 32 — Filesystem posture provider (read-only, bounded walk).
//!
//! Flags: world-writable files outside sanctioned system sets, SUID binaries
//! inventory, `.ssh` directory/key permission violations, home-dir exposure.
//! The walk is depth- and wall-clock-bounded; hitting a bound degrades the
//! lane honestly instead of pretending completeness.
//!
//! Permissions are read from the host's own model: POSIX mode bits on unix, the
//! DACL on Windows (DBT-P36-006). Nothing is synthesised: Windows has no SUID, so
//! that inventory is unix-only, and evidence always shows the mode or the ACE read.

use crate::model::{
    Confidence, EvidenceRef, MAX_FILES_PER_SCAN, MAX_SCAN_MILLIS, SecFinding, Severity,
    sort_and_clamp,
};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Directories whose world-writable entries are sanctioned by the OS baseline.
const SANCTIONED_PREFIXES: [&str; 5] = [
    "/tmp",
    "/private/var/tmp",
    "/System/Volumes/Data/private/tmp",
    "/dev",
    "/Library/Caches",
];

struct WalkBudget {
    files_visited: usize,
    deadline: Instant,
    truncated: bool,
}

fn is_sanctioned(path: &str) -> bool {
    SANCTIONED_PREFIXES
        .iter()
        .any(|p| path == *p || path.starts_with(&format!("{p}/")))
}

/// What one path's permissions show, read from the host's own permission model. Each
/// field is the observed evidence when the condition holds.
struct Posture {
    /// SUID bit set (unix only: Windows has no equivalent, so nothing is synthesised).
    suid: Option<String>,
    /// Writable by everyone.
    world_writable: Option<String>,
    /// Accessible to someone other than the owner (and, on Windows, SYSTEM and
    /// Administrators) — the check OpenSSH applies to `.ssh` and key material.
    exposed: Option<String>,
}

#[cfg(unix)]
mod expected {
    pub const WORLD_WRITABLE: &str = "no group/other write bits";
    pub const SSH_DIR: &str = "0700 on ~/.ssh";
    pub const SSH_KEY: &str = "0600 on keys / authorized_keys";
}

#[cfg(windows)]
mod expected {
    pub const WORLD_WRITABLE: &str = "a DACL with no write-class allow ACE for Everyone, \
         Authenticated Users, Users or Anonymous";
    pub const SSH_DIR: &str = "allow ACEs on .ssh only for its owner, SYSTEM and Administrators";
    pub const SSH_KEY: &str =
        "allow ACEs on key material only for its owner, SYSTEM and Administrators";
}

#[cfg(unix)]
fn posture(_path: &Path, meta: &std::fs::Metadata) -> Result<Posture, String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = meta.permissions().mode() & 0o7777;
    let shown = format!("mode={mode:04o}");
    Ok(Posture {
        suid: (mode & 0o4000 != 0).then(|| shown.clone()),
        world_writable: (mode & 0o002 != 0).then(|| shown.clone()),
        exposed: (mode & 0o077 != 0).then_some(shown),
    })
}

#[cfg(windows)]
fn posture(path: &Path, _meta: &std::fs::Metadata) -> Result<Posture, String> {
    let (owner, dacl) = win::read_dacl(path)?;
    Ok(Posture {
        suid: None,
        world_writable: acl::world_writable(dacl.as_deref()),
        exposed: acl::foreign_access(&owner, dacl.as_deref()),
    })
}

/// DACL verdicts, platform-neutral so they are unit-tested on every host; only the
/// Win32 read in `win` is Windows-specific.
#[cfg(any(windows, test))]
mod acl {
    const FILE_WRITE_DATA: u32 = 0x0000_0002;
    const FILE_APPEND_DATA: u32 = 0x0000_0004;
    const WRITE_DAC: u32 = 0x0004_0000;
    const WRITE_OWNER: u32 = 0x0008_0000;
    const GENERIC_ALL: u32 = 0x1000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;

    /// Everyone, Authenticated Users, BUILTIN\Users, Anonymous.
    const BROAD: [&str; 4] = ["S-1-1-0", "S-1-5-11", "S-1-5-32-545", "S-1-5-7"];
    /// SYSTEM and BUILTIN\Administrators: the principals Win32-OpenSSH also accepts.
    const TRUSTED: [&str; 2] = ["S-1-5-18", "S-1-5-32-544"];

    const NULL_DACL: &str = "NULL DACL (every principal has full access)";

    /// One DACL entry as read. Only ACCESS_ALLOWED/ACCESS_DENIED (and their callback
    /// forms) are carried; no other ACE type grants or denies file access.
    #[derive(Clone, Debug)]
    pub struct Ace {
        pub sid: String,
        pub allow: bool,
        /// INHERIT_ONLY_ACE: applies to children only, never to this object.
        pub inherit_only: bool,
        pub mask: u32,
    }

    /// The write-class rights `mask` grants, with generic rights resolved through the
    /// file generic mapping (FILE_GENERIC_WRITE and FILE_ALL_ACCESS).
    fn write_rights(mask: u32) -> u32 {
        let mut rights = mask & (FILE_WRITE_DATA | FILE_APPEND_DATA | WRITE_DAC | WRITE_OWNER);
        if mask & (GENERIC_WRITE | GENERIC_ALL) != 0 {
            rights |= FILE_WRITE_DATA | FILE_APPEND_DATA;
        }
        if mask & GENERIC_ALL != 0 {
            rights |= WRITE_DAC | WRITE_OWNER;
        }
        rights
    }

    fn shown(ace: &Ace) -> String {
        format!("allow {} mask=0x{:08x}", ace.sid, ace.mask)
    }

    /// `dacl == None` is a NULL DACL. Walks in ACL order, as AccessCheck does: a deny
    /// for the same SID earlier in the list removes the rights it covers.
    pub fn world_writable(dacl: Option<&[Ace]>) -> Option<String> {
        let Some(aces) = dacl else {
            return Some(NULL_DACL.to_string());
        };
        let mut denied: Vec<(&str, u32)> = Vec::new();
        for ace in aces
            .iter()
            .filter(|a| !a.inherit_only && BROAD.contains(&a.sid.as_str()))
        {
            let rights = write_rights(ace.mask);
            if !ace.allow {
                denied.push((&ace.sid, rights));
                continue;
            }
            let masked = denied
                .iter()
                .filter(|(sid, _)| *sid == ace.sid)
                .fold(0, |acc, (_, m)| acc | m);
            if rights & !masked != 0 {
                return Some(shown(ace));
            }
        }
        None
    }

    /// Any allow ACE for a principal other than the owner, SYSTEM or Administrators.
    pub fn foreign_access(owner: &str, dacl: Option<&[Ace]>) -> Option<String> {
        let Some(aces) = dacl else {
            return Some(NULL_DACL.to_string());
        };
        aces.iter()
            .find(|a| {
                a.allow
                    && !a.inherit_only
                    && a.mask != 0
                    && a.sid != owner
                    && !TRUSTED.contains(&a.sid.as_str())
            })
            .map(shown)
    }
}

#[cfg(windows)]
mod win {
    //! Declared locally rather than through the `windows` crate: adding that
    //! dependency here changes Cargo.lock, which only the dependency lane may do.
    //! Signatures and constants are the documented Win32 ones (winnt.h, aclapi.h,
    //! sddl.h).

    use super::acl::Ace;
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt as _;
    use std::path::Path;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn GetNamedSecurityInfoW(
            object_name: *const u16,
            object_type: i32,
            security_info: u32,
            owner: *mut *mut c_void,
            group: *mut *mut c_void,
            dacl: *mut *mut c_void,
            sacl: *mut *mut c_void,
            descriptor: *mut *mut c_void,
        ) -> u32;
        fn GetAclInformation(acl: *const c_void, info: *mut c_void, len: u32, class: i32) -> i32;
        fn GetAce(acl: *const c_void, index: u32, ace: *mut *mut c_void) -> i32;
        fn ConvertSidToStringSidW(sid: *const c_void, string_sid: *mut *mut u16) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(mem: *mut c_void) -> *mut c_void;
    }

    const SE_FILE_OBJECT: i32 = 1;
    const OWNER_SECURITY_INFORMATION: u32 = 0x1;
    const DACL_SECURITY_INFORMATION: u32 = 0x4;
    const ACL_SIZE_INFORMATION_CLASS: i32 = 2;

    #[repr(C)]
    #[derive(Default)]
    struct AclSizeInformation {
        ace_count: u32,
        acl_bytes_in_use: u32,
        acl_bytes_free: u32,
    }

    /// ACE_HEADER, then (for the types read below) a u32 ACCESS_MASK and the SID.
    #[repr(C)]
    struct MaskAndSidAce {
        ace_type: u8,
        ace_flags: u8,
        _ace_size: u16,
        mask: u32,
        sid_start: u32,
    }

    // ACE types whose body is Mask + SID. The callback (conditional) forms are read as
    // unconditional: that can over-report a grant, never hide one.
    const ACCESS_ALLOWED: u8 = 0;
    const ACCESS_DENIED: u8 = 1;
    const ACCESS_ALLOWED_CALLBACK: u8 = 9;
    const ACCESS_DENIED_CALLBACK: u8 = 10;
    const INHERIT_ONLY_ACE: u8 = 0x08;

    /// Frees LocalAlloc'd memory returned by the security APIs on every exit path.
    struct LocalMem(*mut c_void);
    impl Drop for LocalMem {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    LocalFree(self.0);
                }
            }
        }
    }

    fn os_error(call: &str) -> String {
        format!("{call}: {}", std::io::Error::last_os_error())
    }

    fn sid_string(sid: *const c_void) -> Result<String, String> {
        let mut raw: *mut u16 = std::ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(sid, &mut raw) } == 0 || raw.is_null() {
            return Err(os_error("ConvertSidToStringSidW"));
        }
        let _free = LocalMem(raw.cast());
        let len = (0..).take_while(|&i| unsafe { *raw.add(i) } != 0).count();
        String::from_utf16(unsafe { std::slice::from_raw_parts(raw, len) })
            .map_err(|e| format!("SID string: {e}"))
    }

    /// The owner SID and the DACL exactly as stored; `None` is a NULL DACL.
    pub fn read_dacl(path: &Path) -> Result<(String, Option<Vec<Ace>>), String> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut owner: *mut c_void = std::ptr::null_mut();
        let mut dacl: *mut c_void = std::ptr::null_mut();
        let mut descriptor: *mut c_void = std::ptr::null_mut();
        let status = unsafe {
            GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        // `owner` and `dacl` point into the descriptor; it outlives every read below.
        let _free = LocalMem(descriptor);
        if status != 0 {
            return Err(format!(
                "GetNamedSecurityInfoW: {}",
                std::io::Error::from_raw_os_error(status as i32)
            ));
        }
        let owner = if owner.is_null() {
            String::new()
        } else {
            sid_string(owner)?
        };
        if dacl.is_null() {
            return Ok((owner, None));
        }
        let mut info = AclSizeInformation::default();
        let ok = unsafe {
            GetAclInformation(
                dacl,
                (&raw mut info).cast(),
                size_of::<AclSizeInformation>() as u32,
                ACL_SIZE_INFORMATION_CLASS,
            )
        };
        if ok == 0 {
            return Err(os_error("GetAclInformation"));
        }
        let mut aces = Vec::with_capacity(info.ace_count as usize);
        for index in 0..info.ace_count {
            let mut raw: *mut c_void = std::ptr::null_mut();
            if unsafe { GetAce(dacl, index, &mut raw) } == 0 || raw.is_null() {
                return Err(os_error("GetAce"));
            }
            let ace = raw.cast::<MaskAndSidAce>();
            let (ace_type, ace_flags) = unsafe { ((*ace).ace_type, (*ace).ace_flags) };
            let allow = match ace_type {
                ACCESS_ALLOWED | ACCESS_ALLOWED_CALLBACK => true,
                ACCESS_DENIED | ACCESS_DENIED_CALLBACK => false,
                // Object ACEs (directory-service objects) and audit or label ACEs
                // grant no file access, and their SID sits at another offset.
                _ => continue,
            };
            let mask = unsafe { (*ace).mask };
            let sid = sid_string(unsafe { &raw const (*ace).sid_start }.cast())?;
            aces.push(Ace {
                sid,
                allow,
                inherit_only: ace_flags & INHERIT_ONLY_ACE != 0,
                mask,
            });
        }
        Ok((owner, Some(aces)))
    }
}

/// Every regular file under `dir`, recursively, within the budget. Reparse points are
/// skipped, never followed — see the note in secrets.rs::collect_files.
fn walk(dir: &Path, budget: &mut WalkBudget, files: &mut Vec<PathBuf>) {
    if budget.files_visited >= MAX_FILES_PER_SCAN || Instant::now() > budget.deadline {
        budget.truncated = true;
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        budget.files_visited += 1;
        if budget.files_visited >= MAX_FILES_PER_SCAN || Instant::now() > budget.deadline {
            budget.truncated = true;
            return;
        }
        let Ok(meta) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        if crate::scope::is_reparse_point(&meta) {
            continue;
        }
        if meta.is_dir() {
            walk(&p, budget, files);
            if budget.truncated {
                return;
            }
        } else if meta.is_file() {
            files.push(p);
        }
    }
}

/// Collects candidate regular-file paths under `roots` within bounds.
fn collect_files(roots: &[String]) -> (Vec<PathBuf>, bool) {
    let mut out = Vec::new();
    let mut budget = WalkBudget {
        files_visited: 0,
        deadline: Instant::now() + std::time::Duration::from_millis(MAX_SCAN_MILLIS),
        truncated: false,
    };
    for root in roots {
        let p = Path::new(root);
        match std::fs::metadata(p) {
            Ok(m) if m.is_file() => {
                out.push(p.to_path_buf());
                budget.files_visited += 1;
            }
            _ => walk(p, &mut budget, &mut out),
        }
        if budget.truncated {
            break;
        }
    }
    (out, budget.truncated)
}

pub fn audit_filesystem(roots: &[String]) -> Result<Vec<SecFinding>, String> {
    let (files, truncated) = collect_files(roots);
    let mut findings = Vec::new();
    // Paths whose permissions could not be read: never reported as clean.
    let mut unreadable: Vec<String> = Vec::new();
    for f in &files {
        let Ok(meta) = std::fs::symlink_metadata(f) else {
            continue;
        };
        let shown = f.display().to_string();
        let posture = match posture(f, &meta) {
            Ok(p) => p,
            Err(e) => {
                unreadable.push(format!("{shown}: {e}"));
                continue;
            }
        };
        // SUID inventory (informational, bounded).
        if let Some(observed) = posture.suid
            && let Some(fd) = SecFinding::try_new(
                "SEC-FS-002",
                "fs.suid_inventory",
                Severity::Advisory,
                vec![EvidenceRef {
                    fact: format!("SUID binary present: {shown}"),
                    observed,
                    expected_or_threshold: "inventory only — review against baseline".to_string(),
                    source_location: shown.clone(),
                }],
                None,
                "sec.fs.suidInventory",
                Confidence::Exact,
            )
        {
            findings.push(fd);
        }
        // World-writable outside sanctioned sets.
        if let Some(observed) = posture.world_writable
            && !is_sanctioned(&shown)
            && let Some(fw) = SecFinding::try_new(
                "SEC-FS-001",
                "fs.world_writable",
                Severity::Medium,
                vec![EvidenceRef {
                    fact: format!("world-writable file outside sanctioned set: {shown}"),
                    observed,
                    expected_or_threshold: expected::WORLD_WRITABLE.to_string(),
                    source_location: shown.clone(),
                }],
                Some("CIS L1 6.1.1"),
                "sec.fs.worldWritable",
                Confidence::Exact,
            )
        {
            findings.push(fw);
        }
    }
    // .ssh posture per root that IS a home-like dir.
    for root in roots {
        let ssh_dir = Path::new(root).join(".ssh");
        // symlink_metadata, not metadata: a `.ssh` that is itself a junction must be
        // skipped rather than silently resolved to someone else's key material.
        let Ok(meta) = std::fs::symlink_metadata(&ssh_dir) else {
            continue;
        };
        if crate::scope::is_reparse_point(&meta) || !meta.is_dir() {
            continue;
        }
        match posture(&ssh_dir, &meta) {
            Err(e) => unreadable.push(format!("{}: {e}", ssh_dir.display())),
            Ok(Posture {
                exposed: Some(observed),
                ..
            }) => {
                if let Some(f) = SecFinding::try_new(
                    "SEC-FS-003",
                    "fs.ssh_dir_perms",
                    Severity::High,
                    vec![EvidenceRef {
                        fact: format!(".ssh directory over-permissive: {}", ssh_dir.display()),
                        observed: format!("dir {observed}"),
                        expected_or_threshold: expected::SSH_DIR.to_string(),
                        source_location: ssh_dir.display().to_string(),
                    }],
                    None,
                    "sec.fs.sshDirPerms",
                    Confidence::Exact,
                ) {
                    findings.push(f);
                }
            }
            Ok(_) => {}
        }
        if let Ok(entries) = std::fs::read_dir(&ssh_dir) {
            for e in entries.flatten() {
                let p = e.path();
                let Ok(fm) = std::fs::symlink_metadata(&p) else {
                    continue;
                };
                if crate::scope::is_reparse_point(&fm) || !fm.is_file() {
                    continue;
                }
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                // A `.pub` key is public: ssh-keygen writes it 0644 and OpenSSH never
                // checks its permissions.
                let is_key = (name.starts_with("id_") && !name.ends_with(".pub"))
                    || name.ends_with(".pem")
                    || name == "authorized_keys";
                if !is_key {
                    continue;
                }
                let observed = match posture(&p, &fm) {
                    Err(err) => {
                        unreadable.push(format!("{}: {err}", p.display()));
                        continue;
                    }
                    Ok(Posture {
                        exposed: Some(observed),
                        ..
                    }) => observed,
                    Ok(_) => continue,
                };
                if let Some(fk) = SecFinding::try_new(
                    "SEC-FS-004",
                    "fs.ssh_key_perms",
                    Severity::Critical,
                    vec![EvidenceRef {
                        fact: format!("SSH key material over-permissive: {}", p.display()),
                        observed: format!("file {observed}"),
                        expected_or_threshold: expected::SSH_KEY.to_string(),
                        source_location: p.display().to_string(),
                    }],
                    None,
                    "sec.fs.sshKeyPerms",
                    Confidence::Exact,
                ) {
                    findings.push(fk);
                }
            }
        }
    }
    let mut all = sort_and_clamp(findings);
    if truncated {
        // Honest truncation marker as an Advisory finding with evidence naming
        // the bound that was hit.
        if let Some(ft) = SecFinding::try_new(
            "SEC-FS-900",
            "fs.scan_truncated",
            Severity::Advisory,
            vec![EvidenceRef {
                fact: format!(
                    "filesystem scan hit a bound after {} entries or {}ms",
                    MAX_FILES_PER_SCAN, MAX_SCAN_MILLIS
                ),
                observed: "truncated=true".to_string(),
                expected_or_threshold: "full coverage within bounds".to_string(),
                source_location: roots.first().cloned().unwrap_or_default(),
            }],
            None,
            "sec.fs.scanTruncated",
            Confidence::Inferred,
        ) {
            all.push(ft);
        }
    }
    if let Some(first) = unreadable.first() {
        // Coverage is partial in the same sense as a hit bound: an unread permission
        // is not a clean one, so the compliance view must not score it as a pass.
        if let Some(fu) = SecFinding::try_new(
            "SEC-FS-901",
            "fs.scan_truncated",
            Severity::Advisory,
            vec![EvidenceRef {
                fact: format!(
                    "permissions of {} entries could not be read; first: {first}",
                    unreadable.len()
                ),
                observed: format!("unreadable={}", unreadable.len()),
                expected_or_threshold: "every entry's permissions read".to_string(),
                source_location: roots.first().cloned().unwrap_or_default(),
            }],
            None,
            "sec.fs.scanTruncated",
            Confidence::Exact,
        ) {
            all.push(fu);
        }
    }
    Ok(sort_and_clamp(all))
}

#[cfg(test)]
mod tests {
    use super::acl::{Ace, foreign_access, world_writable};

    const OWNER: &str = "S-1-5-21-1-2-3-1001";

    fn allow(sid: &str, mask: u32) -> Ace {
        Ace {
            sid: sid.into(),
            allow: true,
            inherit_only: false,
            mask,
        }
    }

    fn deny(sid: &str, mask: u32) -> Ace {
        Ace {
            allow: false,
            ..allow(sid, mask)
        }
    }

    /// The ACL a file inherits in a user profile: owner, SYSTEM, Administrators, all
    /// FILE_ALL_ACCESS (0x001f01ff).
    fn private_acl() -> Vec<Ace> {
        vec![
            allow(OWNER, 0x001f_01ff),
            allow("S-1-5-18", 0x001f_01ff),
            allow("S-1-5-32-544", 0x001f_01ff),
        ]
    }

    #[test]
    fn everyone_write_is_world_writable_and_names_the_ace() {
        let mut dacl = private_acl();
        // icacls *S-1-1-0:(W) = FILE_GENERIC_WRITE.
        dacl.push(allow("S-1-1-0", 0x0012_0116));
        assert_eq!(
            world_writable(Some(&dacl)).as_deref(),
            Some("allow S-1-1-0 mask=0x00120116")
        );
    }

    #[test]
    fn every_broad_principal_and_write_class_right_counts() {
        for sid in ["S-1-1-0", "S-1-5-11", "S-1-5-32-545", "S-1-5-7"] {
            // FILE_WRITE_DATA, FILE_APPEND_DATA, WRITE_DAC, WRITE_OWNER, GENERIC_ALL,
            // GENERIC_WRITE.
            for mask in [0x2, 0x4, 0x0004_0000, 0x0008_0000, 0x1000_0000, 0x4000_0000] {
                assert!(
                    world_writable(Some(&[allow(sid, mask)])).is_some(),
                    "{sid} {mask:#x}"
                );
            }
        }
    }

    #[test]
    fn read_only_broad_grants_and_trusted_principals_are_not_world_writable() {
        let mut dacl = private_acl();
        // Users:(RX) and GENERIC_READ|GENERIC_EXECUTE are not write-class.
        dacl.push(allow("S-1-5-32-545", 0x0012_00a9));
        dacl.push(allow("S-1-1-0", 0xa000_0000));
        assert_eq!(world_writable(Some(&dacl)), None);
    }

    #[test]
    fn inherit_only_ace_is_ignored() {
        let mut dacl = private_acl();
        dacl.push(Ace {
            inherit_only: true,
            ..allow("S-1-5-11", 0x1000_0000)
        });
        assert_eq!(world_writable(Some(&dacl)), None);
        assert_eq!(foreign_access(OWNER, Some(&dacl)), None);
    }

    #[test]
    fn earlier_deny_for_the_same_sid_cancels_only_what_it_covers() {
        let covered = [deny("S-1-1-0", 0x6), allow("S-1-1-0", 0x6)];
        assert_eq!(world_writable(Some(&covered)), None);
        // A deny AFTER the allow never applies: AccessCheck stops at the grant.
        let late = [allow("S-1-1-0", 0x6), deny("S-1-1-0", 0x6)];
        assert!(world_writable(Some(&late)).is_some());
        // A deny for another SID does not cover Everyone.
        let other = [deny("S-1-5-32-545", 0x6), allow("S-1-1-0", 0x6)];
        assert!(world_writable(Some(&other)).is_some());
    }

    #[test]
    fn null_dacl_is_world_writable_and_exposed() {
        assert!(world_writable(None).unwrap().starts_with("NULL DACL"));
        assert!(
            foreign_access(OWNER, None)
                .unwrap()
                .starts_with("NULL DACL")
        );
    }

    #[test]
    fn owner_system_and_administrators_only_is_not_exposed() {
        assert_eq!(foreign_access(OWNER, Some(&private_acl())), None);
        // An empty DACL grants nothing to anyone.
        assert_eq!(foreign_access(OWNER, Some(&[])), None);
    }

    #[test]
    fn any_other_principal_even_read_only_exposes_key_material() {
        let mut dacl = private_acl();
        dacl.push(allow("S-1-5-32-545", 0x0012_0089));
        assert_eq!(
            foreign_access(OWNER, Some(&dacl)).as_deref(),
            Some("allow S-1-5-32-545 mask=0x00120089")
        );
        // A second user is as foreign as a broad group.
        let other_user = [allow("S-1-5-21-1-2-3-1002", 0x0012_0089)];
        assert!(foreign_access(OWNER, Some(&other_user)).is_some());
        // Deny ACEs grant nothing.
        assert_eq!(
            foreign_access(OWNER, Some(&[deny("S-1-1-0", 0x001f_01ff)])),
            None
        );
    }
}
