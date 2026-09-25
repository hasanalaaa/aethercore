# P75 lane `keygen-rng` — evidence

Branch `lane/keygen-rng`, based on `origin/main` `f22d285`. Files touched:
`apps/aetherctl/src/offline.rs`, `apps/aetherctl/tests/keys_generate.rs` (new),
`apps/aetherctl/tests/keys_generate_windows_rng.rs` (new), this file, and
`MANIFEST.sha256` (seal). `Cargo.toml` and `Cargo.lock` are unchanged.

## Proposed new row (the lead assigns the id)

| `DBT-P75-NNN` | **`aetherctl keys generate` on Windows took its Ed25519 seed from `<current drive>:\dev\urandom`, a file any local user can create; it also overwrote an existing seed file and claimed `0600` it had not set** | **CLOSED by P75 lane `keygen-rng`** — seed from BCryptGenRandom on Windows, `create_new` refuses an existing path, unix file created `0600` in the creating open, `permissions` reported as measured (`inherited` on Windows) | see below | the Windows runner |

## 1. Seed source (HIGH)

**Defect, confirmed by reading the code.** `keys_generate` opened the literal path
`"/dev/urandom"`. On Windows a rooted path without a drive letter resolves against
the current drive, so it read `<drive>:\dev\urandom`. Absent (the normal case), the
command always failed with exit 8. Present, the attacker's bytes became the seed,
and those keys sign `EXPORT_V1` exports. Authenticated users may create folders at
a drive root by default.

**Change.** `os_random()`: `/dev/urandom` on unix (unchanged, and root-owned), and
on Windows a local `unsafe extern "system"` `BCryptGenRandom` from `bcrypt.dll` with
`BCRYPT_USE_SYSTEM_PREFERRED_RNG` (0x2) and a null algorithm handle, which is the
local-extern pattern `consent-broker` and `install-hardener` use. A non-zero
NTSTATUS is an error and generation stops before anything is written.
`std::random` was checked first: it is still unstable on 1.97.1 (`E0658`, issue
130703).

**Proof.** `tests/keys_generate_windows_rng.rs`, `#[cfg(windows)]`, is its own test
binary. It plants `<drive>\dev\urandom` = `0x41` × 4096 on the child's drive, runs
the real binary with that working directory, and asserts the seed is not `"41"` × 32.
It removes only what it created, and refuses to run if the file already exists.

* The Windows verdict is CI's `cargo test --workspace` on `windows-2025`; see the PR
  checks at the branch head.
* **Red before the fix, measured on Windows:** CI `36110394452` at test-only
  commit `026c784e347bdfb11e6e2923efabda06b2adb912` failed in
  `seed_never_comes_from_a_planted_dev_urandom`: the signing seed was the
  planted `C:\\dev\\urandom` bytes (`"41"` repeated 32 times on both sides of
  `assert_ne!`). The branch was based on green `main` `2e7485a`; it changed
  only the test and source manifest. The test planted the file successfully,
  so this is the attacker-controlled seed path, not a setup failure.
* Local, macOS: the Windows function and the test file were type-checked and linted
  with `clippy-driver --edition 2024 --target x86_64-pc-windows-gnu -D warnings`
  (exit 0). `cargo clippy --target x86_64-pc-windows-msvc` cannot run here, since
  `aetherctl` pulls `libsqlite3-sys`.

## 2. Overwrite (data integrity)

**Defect, reproduced on macOS at `9747fe3`.** Two runs of
`keys generate --out repro/k.key`: both exit 0, the file went from `60f16e5d…` to
`fba8dfa1…`, and the first key was gone. `fs::write` also followed a symlink at
`--out`.

**Change.** `OpenOptions::create_new(true)` (`O_CREAT|O_EXCL`, `CREATE_NEW`). An
existing path gives exit 8, `LocalIo`, `message_key` `local.keys.exists`, and is not
touched. A dangling symlink is refused too (measured). No flag was added to force an
overwrite.

**Proof.** `keys_generate.rs::refuses_to_overwrite_an_existing_seed_file`. Before the
fix (macOS): `left: Some(0) right: Some(8)` with `"generated":true`. After:
`1 passed`.

## 3. Permissions reported

**Defect.** The code wrote with the default mode, then ran `chmod 0600` and dropped
its result. That left the seed at `0644` between the two calls, and the output said
`"permissions":"0600"` even if the chmod failed. On Windows it said `0600` while
setting nothing.

**Change.** On unix, `OpenOptionsExt::mode(0o600)` goes into the creating open, and
`permissions` is read back with `fstat` on the open handle. On Windows it is
`"inherited"`: the file gets its directory's ACL, and nothing in this code narrows
it. The JSON key and its type did not change.

**Proof.**
* unix `seed_file_is_owner_only_and_the_report_matches_it`: mode `0600` and
  `"0600"`. **It passes on the old code as well**, because the late chmod lands. It
  pins the result and cannot catch the race window. Measured instead: under
  `umask 0277` the output now reads `"0400"`, which is the file's real mode.
* Windows `windows_reports_the_inherited_acl_not_a_unix_mode`: on the code before
  this commit it reads `"0600"` and fails. CI is the verdict.

## 4. Module doc

`offline.rs:1-4` said "STRICTLY read-only … no filesystem writes". That has been
false since P29. It now names the one writer. The change is comment-only.

## Local proof (macOS, from `phase21-workspace/`)

```
cargo fmt --all -- --check                                              exit 0
cargo clippy -p aetherctl --all-targets --locked --no-deps -- -D warnings  Finished, 0 warnings
cargo test -p aetherctl --locked        30 unit + keys_generate 2 passed
cargo test --workspace --locked         651 passed, 0 failed, 1 ignored across 137 suites (WIP tree, same source)
python3 scripts/static_validate.py      "failed": []  (347 checks)
python3 scripts/test_gate_readers.py    all 14 readers fail closed
python3 scripts/ps_marker_scan.py       total assertions=234 failed=0 unmeasured=3 (pre-existing, not these files)
python3 scripts/p33-live-proofs.py      exit 0; key from `keys generate` signs, and GD-4 verifies signed=true
python3 scripts/source_seal.py          OK
```

Without `--no-deps`, clippy fails on `aethercore-performance-telemetry`. That is the
pre-existing macOS finding lane `mac-clippy` owns, and it is not from this lane.

## Text-gate delta: needs a file outside this lane

Each phase audit was run before and after. The final failure lists:

| audit | before | after | new |
|---|---|---|---|
| phase28 | 11 | 11 | `p28-mutation-guard:offline.rs` now names `OpenOptions` instead of `fs::write` |
| phase29 / 30 / 31 | 8 / 10 / 12 | 10 / 12 / 14 | `p28-mutation-guard:offline.rs … ['OpenOptions']` (the P29 filter matches only `.*fs::write.*`) and `p29-mutation-guard-scoped` (it requires the text `fs::write` in `offline.rs`) |
| phase32 / 33 / 35 | 5 / 5 / 3 | 5 / 5 / 3 | none (P32 supersedes `^p28-mutation-guard:offline\.rs: .*$`) |
| phase34 | crashes: `AetherCore-Phase33-Master-Delivery.zip` missing | same | — |

None of these audits run in CI. All of them were already `FAIL` on `main`.
`p29-mutation-guard-scoped` is tied to the API name of the old writer. That writer
is the unsafe one, so keeping `fs::write`, or adding a decoy token, would reintroduce
the defect or game the gate. Neither was done. The proposed gate change is for
`scripts/phase29-adversarial-audit.py`, which is outside this lane, in its own
commit:

* filter `^p28-mutation-guard:offline\.rs: .*fs::write.*$` →
  `^p28-mutation-guard:offline\.rs: .*(fs::write|OpenOptions).*$`
* `p29-mutation-guard-scoped`: require `create_new(true)` instead of `fs::write`
  (still no `std::process::Command`). This asserts the no-overwrite invariant, so it
  tightens the gate.

## Not done

* Windows DACL on the seed file: it inherits its directory's ACL, which is reported
  truthfully but not narrowed. Narrowing it needs `CreateFileW` with a security
  descriptor. Proposed as a follow-up row.
* `i18n.rs` `ok.keysGenerated` still says "(seed 0600)" in EN and AR. It is outside
  this lane and rendered nowhere (`main.rs:47` binds the language to
  `_active_language`), but it is stale on Windows.
* The new `local.keys.exists` detail is English-only, like every `local.*` detail in
  this module. The catalogs hold no `local.*` keys.
