# P75 lane `fs-acl` — evidence

The branch is `lane/fs-acl`. Scope: `crates/security-audit/**`; no dependency or
lockfile change. `DBT-P36-006` is the filesystem posture row. The CVE and
firewall changes correct two further unsupported-state claims.

## Filesystem permissions (`DBT-P36-006`)

The old Windows path substituted mode `0666` for every file, then declared
every one world-writable at Exact confidence. It also skipped files directly
beneath the scan root. Windows red-before CI `36057784591` at `23f7169`
failed all three new posture tests. The implementation now reads the DACL with
`GetNamedSecurityInfoW`, walks applicable ACEs, and reports the SID and mask
of broad write grants. A null DACL is exposed; a missing DACL read yields
unavailable evidence instead of invented mode bits. Root-level files are
included. Callback deny ACEs do not hide an unconditional broad grant, covered
by `conditional_deny_cannot_hide_an_unconditional_broad_write_grant`.

## CVE census

The old `cve` lane reported `Ok(0)` when no supported package census could run.
`unavailable_census_reason` now maps that to `NotAvailable`; the unit test
`unsupported_package_census_cannot_report_zero_cves` fails on the old code
and passes on this branch. A zero remains meaningful only when at least one
census source actually ran against the verified local database join.

## Firewall configuration

The old Windows path looked for Unix pf/ufw/nftables files and produced a
Unix-specific absence reason. A test-only probe run `36111505675` failed
earlier in an unrelated cleaner timeout, so it did **not** measure firewall
red-before. Probe `36177125187` at `38db6f7` runs the focused Windows
assertion before the workspace suite; its result must be read before claiming
the Windows red-before.

The new code reads `EnableFirewall` as a DWORD for Domain, Private and Public
profiles under `HKLM`, taking a policy value first and a local configuration
value only when policy is absent. Per-profile read errors or missing settings
are reported as unavailable. The finding names each observed profile, value
and registry path. It explicitly says that registry configuration is not the
effective firewall state; no live rule or packet-filter claim is made.

## Local verification

From `phase21-workspace/`, with debug info and incremental builds disabled:

* `cargo test -p aethercore-security-audit --locked`: 58 passed, 0 failed.
* `cargo clippy -p aethercore-security-audit --all-targets --locked --no-deps -- -D warnings`: pass.
* `cargo clippy -p aethercore-security-audit --lib --target x86_64-pc-windows-msvc --locked --no-deps -- -D warnings`: pass (Windows library type-check; the runner remains the runtime verdict).

Integration CI at the final PR head and the main merge SHA is required before
this lane is closed. No Windows effective firewall state was measured.
