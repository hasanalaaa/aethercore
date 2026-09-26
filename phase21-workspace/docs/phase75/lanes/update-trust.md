# P75 lane `update-trust` (Wave 2, release-authority part) — evidence (ledger `DBT-P75-035`…`037`)

Branch `lane/update-trust`. Scope used: `crates/release-authority/src/lib.rs` only. No dependency
or lockfile change. Each test failed on `main` (`a013de5`) before its fix.

| row | commit | what was wrong | now | red-before |
|---|---|---|---|---|
| `DBT-P75-035` | `c6d2530` | `KeyRotation::authorize` checked the old key with `active_key(old, None)`, skipping its validity window: an expired key could mint a successor | `authorize(keyring, now_epoch)` checks the window | `an_expired_key_cannot_authorize_a_rotation`: `authorize` returned `Ok` |
| `DBT-P75-036` | `bbab01e` | `compare_versions("1.2", "1.2.0")` was -1: a re-release of the same version passed as an upgrade | components are zero-padded | `trailing_zero_components_compare_equal`: `left: -1, right: 0` |
| `DBT-P75-037` | `360cd87` | an offline source rejected `..` only in absolute paths (`is_absolute() && contains("..")`) | any `ParentDir` component is refused; `a..b` names are fine | `offline_source_with_a_parent_component_is_refused` |

## The rest of the lane (second part, branch `lane/update-trust-2`, from `main` `821163f`)

| row | commit | what was wrong | now | red-before / measured |
|---|---|---|---|---|
| — (Stage 0 finding, **disproved**) | none | "a 60 s whole-request timeout means any realistically sized update fails to download" (`update-download/src/lib.rs:40`, INFERRED) | unchanged: reqwest 0.13.4's blocking `ClientBuilder::timeout` is applied per wait — the send, then each `read` (`blocking/response.rs:440`, `wait::timeout(self.body_mut().read(buf), timeout)`), not to the transfer | a local server trickled 8 bytes over 3.2 s to a client with `timeout(1 s)`: `elapsed=3.276579041s result=Ok(8)`. A stall longer than 60 s fails; a slow download does not. Measurement test not kept |
| `DBT-P75-053` | `a61d663` | `load_with_build` called `recover_execution_guard()?`; a durable guard whose staged installer no longer verified failed the coordinator, composition and every service start until the guard's 2-hour TTL expired | the guard is abandoned (a running image cannot be deleted or modified on Windows, so a file that fails verification is not executing), the update is reported Failed, the service starts; a database error still propagates | `an_unrecoverable_execution_guard_does_not_block_startup`: `Io(Os { code: 2, kind: NotFound })` |
| `DBT-P75-054` | `366eff8` | the support-bundle sanitizer let through a user path escaped twice, host names, IPv4/IPv6 addresses (with or without a port), MAC addresses, and serials under any key but four exact names | each is redacted; strings under a `*version*` key are kept (10.1.19.3 is not an address); loopback/unspecified kept | `escaped_paths_hosts_addresses_and_every_serial_are_redacted`: `Alice leaked` (and every other value) |
| `DBT-P75-055` (**OPEN**, owner) | — | `SupportPrivacyReport` has no counter for host names, IPs or MACs | counted with account identifiers (hosts, IPs) and hardware serials (MACs); a field of their own is a wire-contract change (`support_bundle.proto`) | — |

## Local verification, second part (macOS)

* `cargo test -p aethercore-update-engine --locked`: 19 pass. `cargo test -p aethercore-support-bundle --locked`: 14 pass.
* `cargo clippy -p aethercore-update-engine -p aethercore-support-bundle --all-targets --locked -- -D warnings`: clean;
  `aethercore-support-bundle` also clean for `--target x86_64-pc-windows-msvc` (update-engine pulls
  `libsqlite3-sys`; the Windows CI job is its compile verdict).

## Local verification (macOS)

* `cargo test -p aethercore-release-authority --locked`: 15 pass.
* `cargo clippy -p aethercore-release-authority --all-targets --locked -- -D warnings`: clean, host and
  `--target x86_64-pc-windows-msvc`. `cargo check --workspace --locked`: clean.
