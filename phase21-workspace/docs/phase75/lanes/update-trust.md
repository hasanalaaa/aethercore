# P75 lane `update-trust` (Wave 2, release-authority part) — evidence (ledger `DBT-P75-035`…`037`)

Branch `lane/update-trust`. Scope used: `crates/release-authority/src/lib.rs` only. No dependency
or lockfile change. Each test failed on `main` (`a013de5`) before its fix.

| row | commit | what was wrong | now | red-before |
|---|---|---|---|---|
| `DBT-P75-035` | `c6d2530` | `KeyRotation::authorize` checked the old key with `active_key(old, None)`, skipping its validity window: an expired key could mint a successor | `authorize(keyring, now_epoch)` checks the window | `an_expired_key_cannot_authorize_a_rotation`: `authorize` returned `Ok` |
| `DBT-P75-036` | `bbab01e` | `compare_versions("1.2", "1.2.0")` was -1: a re-release of the same version passed as an upgrade | components are zero-padded | `trailing_zero_components_compare_equal`: `left: -1, right: 0` |
| `DBT-P75-037` | `360cd87` | an offline source rejected `..` only in absolute paths (`is_absolute() && contains("..")`) | any `ParentDir` component is refused; `a..b` names are fine | `offline_source_with_a_parent_component_is_refused` |

## Not done in this lane

The rest of `update-trust` in `AMBITION.md` §3: the download timeout, the staged-exe failure
blocking service start for 2 h, and support-bundle redaction leaks (escaped paths, host, IP, MAC,
serials). They touch `update-download`, `update-engine` and `support-bundle` and need Windows
evidence.

## Local verification (macOS)

* `cargo test -p aethercore-release-authority --locked`: 15 pass.
* `cargo clippy -p aethercore-release-authority --all-targets --locked -- -D warnings`: clean, host and
  `--target x86_64-pc-windows-msvc`. `cargo check --workspace --locked`: clean.
