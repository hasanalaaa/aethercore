//! Phase 39 — owner-scoped target authorization.
//!
//! `operations.proto` has always said of `RunSecurityAuditRequest.targets_json`:
//! *"Owner-scoped allowlist is enforced by the router."* This module is that
//! allowlist. It exists because the maintenance service runs as LocalSystem and its
//! named pipe grants Authenticated Users by design, so a caller-named absolute path is
//! caller-supplied SCOPE at a privilege boundary, not merely caller-supplied input.
//!
//! The decision is deliberately NOT taken from the request. `OwnerScope` is built from
//! roots the OS reported for the calling principal's own token; the request can only
//! ever narrow within them.
//!
//! Four rules, in order, per named path:
//!
//! 1. no `..` component (kept from `validate_targets`, and still cheapest first);
//! 2. absolute — a relative path would resolve against the SERVICE's working
//!    directory, which is not the caller's scope by any reading;
//! 3. contained in one of the caller's roots, compared component-wise (so
//!    `C:\Users\alice-evil` is not "inside" `C:\Users\alice`) and case-insensitively on
//!    Windows (where the filesystem is);
//! 4. no symlink or Windows reparse point on any existing component between that root
//!    and the target. Containment alone is not enough: a junction planted inside a root
//!    the caller genuinely owns would otherwise redirect a LocalSystem read anywhere on
//!    the disk. This is the posture `aethercore-install-hardener`'s `purge-data` takes
//!    for the same reason — refuse the reparse point rather than follow it. That copy is
//!    not shared: the hardener is the deferred LocalSystem custom action and carries
//!    zero workspace dependencies on purpose, and this copy additionally has to handle
//!    unix symlinks. Same posture, two small implementations, both stated.
//!
//! Rule 4 covers the way IN. The providers' own recursion is guarded at its own site
//! (`is_reparse_point`, used by the bounded walkers) so a link found DURING a scan is
//! skipped rather than followed.

use std::path::{Component, Path, PathBuf};

use crate::model::AuditTarget;

/// Why a target was refused. Each variant is a distinct operator answer, so each carries
/// its own wire message key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetDenial {
    /// The path contained a `..` component.
    Traversal { path: String },
    /// The path is relative, or resolves outside every root the caller owns.
    OutsideOwnerScope { path: String },
    /// A symlink / Windows reparse point stands between the owner root and the target.
    ReparsePoint { path: String },
    /// The service could not establish any root for the calling principal, so there is
    /// nothing it is authorized to read on that caller's behalf.
    OwnerScopeUnresolved,
}

impl TargetDenial {
    /// Stable wire message key. `&'static str` so it drops straight into `ErrorInfo`.
    pub fn code(&self) -> &'static str {
        match self {
            TargetDenial::Traversal { .. } => "sec.traversalRejected",
            TargetDenial::OutsideOwnerScope { .. } => "sec.targetOutsideOwnerScope",
            TargetDenial::ReparsePoint { .. } => "sec.reparsePointRefused",
            TargetDenial::OwnerScopeUnresolved => "sec.ownerScopeUnresolved",
        }
    }
}

impl std::fmt::Display for TargetDenial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetDenial::Traversal { path } => write!(f, "traversal rejected: {path}"),
            TargetDenial::OutsideOwnerScope { path } => write!(
                f,
                "target is outside the calling principal's own scope: {path}"
            ),
            TargetDenial::ReparsePoint { path } => write!(
                f,
                "reparse point refused rather than followed out of scope: {path}"
            ),
            TargetDenial::OwnerScopeUnresolved => {
                write!(f, "no audit scope could be established for this principal")
            }
        }
    }
}

impl std::error::Error for TargetDenial {}

/// The roots one principal may name. Built from the OS's answer for that principal's
/// token — never from the request.
#[derive(Clone, Debug, Default)]
pub struct OwnerScope {
    roots: Vec<PathBuf>,
}

impl OwnerScope {
    /// Roots are canonicalised once, here. A root that does not resolve is DROPPED, not
    /// kept as a literal: an unresolvable root can never contain a target, so dropping
    /// it fails closed. An empty scope refuses everything.
    pub fn new<I: IntoIterator<Item = PathBuf>>(roots: I) -> OwnerScope {
        let mut resolved: Vec<PathBuf> = roots
            .into_iter()
            .filter_map(|root| std::fs::canonicalize(root).ok())
            .collect();
        resolved.dedup();
        OwnerScope { roots: resolved }
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
}

/// True when this entry is a Windows reparse point (junction, mount point, symlink) or a
/// unix symlink. Callers must pass `symlink_metadata` — `metadata` has already followed
/// the link and cannot answer the question.
#[cfg(windows)]
pub fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// See the `cfg(windows)` twin.
#[cfg(not(windows))]
pub fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

/// Comparison key for one path component: the `\\?\` canonicalisation prefix removed,
/// and case folded where the filesystem folds case.
fn component_key(component: Component<'_>) -> String {
    let raw = component.as_os_str().to_string_lossy();
    let stripped = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    if cfg!(windows) {
        stripped.to_lowercase()
    } else {
        stripped.to_string()
    }
}

fn keys(path: &Path) -> Vec<String> {
    path.components().map(component_key).collect()
}

/// Component-wise containment. `starts_with` on the string form would accept
/// `C:\Users\alice-evil` as inside `C:\Users\alice`.
fn contained_in(target: &Path, root: &Path) -> bool {
    let root = keys(root);
    let target = keys(target);
    target.len() >= root.len() && target[..root.len()] == root[..]
}

/// Walk the target one component at a time, deciding containment on the RESOLVED form.
///
/// A link ABOVE the caller's roots is resolved and the walk continues — `/var` is a
/// symlink to `/private/var` on macOS, and `C:\Users` can be redirected on Windows, so
/// refusing there would refuse every legitimate target. That is safe precisely because
/// containment is judged on the resolved path: a link cannot fake its way INTO a root.
///
/// A link at or below a root is REFUSED, never followed. That is the escape that matters:
/// a junction planted inside a tree the caller genuinely owns, pointed at someone else's.
///
/// Components that do not exist yet hold nothing to follow; the walk skips them and keeps
/// going, so a target under a directory that has not been created is still authorized on
/// the ancestors that do exist.
fn resolve_within_roots(target: &Path, roots: &[PathBuf]) -> Result<(), TargetDenial> {
    let mut resolved = PathBuf::new();
    let mut inside = false;
    for component in target.components() {
        resolved.push(component.as_os_str());
        if let Ok(meta) = std::fs::symlink_metadata(&resolved)
            && is_reparse_point(&meta)
        {
            if inside {
                return Err(TargetDenial::ReparsePoint {
                    path: resolved.to_string_lossy().into_owned(),
                });
            }
            resolved =
                std::fs::canonicalize(&resolved).map_err(|_| TargetDenial::OutsideOwnerScope {
                    path: target.to_string_lossy().into_owned(),
                })?;
        }
        if !inside && roots.iter().any(|root| contained_in(&resolved, root)) {
            inside = true;
        }
    }
    if inside {
        Ok(())
    } else {
        Err(TargetDenial::OutsideOwnerScope {
            path: target.to_string_lossy().into_owned(),
        })
    }
}

fn authorize_path(path: &str, scope: &OwnerScope) -> Result<(), TargetDenial> {
    if scope.is_empty() {
        return Err(TargetDenial::OwnerScopeUnresolved);
    }
    let candidate = Path::new(path);
    if candidate
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(TargetDenial::Traversal {
            path: path.to_string(),
        });
    }
    if !candidate.is_absolute() {
        // A relative target would resolve against the SERVICE's working directory.
        return Err(TargetDenial::OutsideOwnerScope {
            path: path.to_string(),
        });
    }
    resolve_within_roots(candidate, &scope.roots)
}

/// THE allowlist the wire contract promises. Every path-bearing target is confined to
/// the calling principal's own roots; `FirewallState` names no path (it reads fixed OS
/// config locations for presence only) and so has nothing to authorize.
pub fn authorize_targets(
    targets: &[AuditTarget],
    scope: &OwnerScope,
) -> Result<(), TargetDenial> {
    for target in targets {
        match target {
            AuditTarget::SshdConfig { path }
            | AuditTarget::PasswordPolicy { path }
            | AuditTarget::Sudoers { path } => authorize_path(path, scope)?,
            AuditTarget::FilesystemPaths { paths } | AuditTarget::AuthLogs { paths } => {
                for path in paths {
                    authorize_path(path, scope)?;
                }
            }
            AuditTarget::SecretsDir { dir } => authorize_path(dir, scope)?,
            AuditTarget::FirewallState => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sandbox {
        root: PathBuf,
    }

    impl Sandbox {
        fn new(label: &str) -> Sandbox {
            let root = std::env::temp_dir().join(format!(
                "aethercore-scope-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("owned/nested")).expect("sandbox");
            std::fs::create_dir_all(root.join("foreign")).expect("sandbox");
            std::fs::write(root.join("foreign/credentials"), "AKIAIOSFODNN7EXAMPLE\n")
                .expect("sandbox");
            Sandbox { root }
        }
        fn owned(&self) -> PathBuf {
            self.root.join("owned")
        }
        fn foreign(&self) -> PathBuf {
            self.root.join("foreign")
        }
        fn scope(&self) -> OwnerScope {
            OwnerScope::new([self.owned()])
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn secrets(dir: &Path) -> Vec<AuditTarget> {
        vec![AuditTarget::SecretsDir {
            dir: dir.to_string_lossy().into_owned(),
        }]
    }

    #[test]
    fn an_absolute_path_outside_every_owner_root_is_refused() {
        let sandbox = Sandbox::new("outside");
        let denial = authorize_targets(&secrets(&sandbox.foreign()), &sandbox.scope())
            .expect_err("must refuse");
        assert_eq!(denial.code(), "sec.targetOutsideOwnerScope");
    }

    #[test]
    fn a_sibling_root_with_a_shared_name_prefix_is_not_inside_it() {
        let sandbox = Sandbox::new("sibling");
        // `owned-evil` string-prefixes `owned`; component-wise it does not.
        let sibling = sandbox.root.join("owned-evil");
        std::fs::create_dir_all(&sibling).expect("sibling");
        let denial =
            authorize_targets(&secrets(&sibling), &sandbox.scope()).expect_err("must refuse");
        assert_eq!(denial.code(), "sec.targetOutsideOwnerScope");
    }

    #[test]
    fn a_link_planted_inside_an_owned_root_is_refused_not_followed() {
        let sandbox = Sandbox::new("link");
        let link = sandbox.owned().join("escape");
        #[cfg(unix)]
        std::os::unix::fs::symlink(sandbox.foreign(), &link).expect("plant link");
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(sandbox.foreign(), &link).expect("plant link");
        let denial = authorize_targets(&secrets(&link), &sandbox.scope()).expect_err("must refuse");
        assert_eq!(denial.code(), "sec.reparsePointRefused");
    }

    #[test]
    fn traversal_is_still_refused_and_still_named_as_traversal() {
        let sandbox = Sandbox::new("traversal");
        let target = sandbox.owned().join("../foreign");
        let denial =
            authorize_targets(&secrets(&target), &sandbox.scope()).expect_err("must refuse");
        assert_eq!(denial.code(), "sec.traversalRejected");
    }

    #[test]
    fn a_relative_path_is_refused_because_it_would_resolve_against_the_service_cwd() {
        let sandbox = Sandbox::new("relative");
        let target = AuditTarget::SecretsDir {
            dir: "etc/ssh".to_string(),
        };
        let denial =
            authorize_targets(std::slice::from_ref(&target), &sandbox.scope())
                .expect_err("must refuse");
        assert_eq!(denial.code(), "sec.targetOutsideOwnerScope");
    }

    #[test]
    fn an_empty_scope_refuses_every_path_bearing_target() {
        let denial = authorize_targets(&secrets(Path::new("/")), &OwnerScope::default())
            .expect_err("must refuse");
        assert_eq!(denial.code(), "sec.ownerScopeUnresolved");
    }

    #[test]
    fn every_path_bearing_variant_is_guarded_not_just_secrets_dir() {
        let sandbox = Sandbox::new("variants");
        let outside = sandbox.foreign().to_string_lossy().into_owned();
        let variants = [
            AuditTarget::SshdConfig {
                path: outside.clone(),
            },
            AuditTarget::PasswordPolicy {
                path: outside.clone(),
            },
            AuditTarget::Sudoers {
                path: outside.clone(),
            },
            AuditTarget::FilesystemPaths {
                paths: vec![outside.clone()],
            },
            AuditTarget::AuthLogs {
                paths: vec![outside.clone()],
            },
            AuditTarget::SecretsDir { dir: outside },
        ];
        for variant in variants {
            let denial = authorize_targets(std::slice::from_ref(&variant), &sandbox.scope())
                .expect_err("every path-bearing variant must be guarded");
            assert_eq!(denial.code(), "sec.targetOutsideOwnerScope", "{variant:?}");
        }
    }

    #[test]
    fn the_owner_s_own_tree_is_allowed_including_paths_that_do_not_exist_yet() {
        let sandbox = Sandbox::new("allowed");
        let targets = [
            AuditTarget::SecretsDir {
                dir: sandbox.owned().to_string_lossy().into_owned(),
            },
            AuditTarget::SecretsDir {
                dir: sandbox
                    .owned()
                    .join("nested")
                    .to_string_lossy()
                    .into_owned(),
            },
            AuditTarget::SshdConfig {
                path: sandbox
                    .owned()
                    .join("not/created/yet/sshd_config")
                    .to_string_lossy()
                    .into_owned(),
            },
            AuditTarget::FirewallState,
        ];
        authorize_targets(&targets, &sandbox.scope()).expect("the caller's own scope is allowed");
    }

    #[test]
    fn firewall_state_needs_no_scope_because_it_names_no_path() {
        authorize_targets(&[AuditTarget::FirewallState], &OwnerScope::default())
            .expect("FirewallState carries no caller-named path");
    }
}
