# AetherCore P75 lane contract — read fully before your first edit

Repo: github.com/hasanalaaa/aethercore. Product workspace: `phase21-workspace/` (product paths below are relative to it). CI workflows live at the repo ROOT `.github/workflows/`. Base: `origin/main` at `9747fe3`, green (CI run 35938070817, head_sha = 9747fe3).

You run in your own git worktree. Work only there. Never edit the lead checkout; read the model file from wherever it lives.

## Setup
1. `git fetch origin && git checkout -b lane/<name> origin/main` (unless you are already on a fresh branch from origin/main — then rename it to `lane/<name>`).
2. The embedded model (gitignored, 1.1 GB) is needed by `cargo test --workspace`:
   `ln -s /Users/hasanalaaa/dev/aethercore/phase21-workspace/assets/models/qwen2.5-1.5b-instruct-q4_k_m.gguf phase21-workspace/assets/models/` — never `git add` it (`*.gguf` is ignored).
3. UI lanes only: `pnpm --dir phase21-workspace/apps/ui install --frozen-lockfile`.

## Disk budget — MANDATORY (the disk has ~50–90 GB free for ~12 parallel worktrees)
Export these in EVERY shell that runs cargo (prefix each cargo command, or `export` at the top of each Bash call):
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`
They change only debug info and incremental caches, never behaviour. Do NOT share a target dir with another worktree (binaries uplifted into target/debug would collide and tests spawn them). Build with `-p <crate>` while iterating; run the full `cargo test --workspace --locked` once before the PR. When your PR is green and you have reported, run `cargo clean` in your worktree.

## Method
reproduce → change → prove. A behavioural change adds a test that FAILS without it — show the failure first (run it against unchanged code, or stash the fix). One concern per commit. Commit message = what, why, proof. End every commit message with:
`Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`

## Invariants — never violate
- No network, account, login or telemetry at rest. Insights cite evidence or are dropped. Read-only diagnosis before mutation. EN/AR parity: every new UI/CLI string exists in both catalogs.
- `ASSISTANT_DEADLINE` (20 s) is a promise to the user — never widen it, nor any product deadline, to make a test pass.
- The pipe DACL `(A;;FR;;;AU)(A;;0x00000002;;;AU)` is deliberate. Hex `0x00120003` is forbidden: SDDL strips SYNCHRONIZE (`crates/ipc/src/windows_impl.rs:276`).
- `C:\ProgramData\AetherCore` is re-owned to SYSTEM by the hardener (P74). Never weaken that.
- Never weaken a gate, widen an allowlist, raise a ceiling or edit an expected value. If a gate is wrong, prove it wrong, then fix the gate in its own commit.
- Stay inside your lane's file list. If the fix needs a file outside it, stop and report — another lane owns it.
- Do NOT edit `docs/LEDGER.md`; the lead moves rows at merge time. Write your evidence to `docs/phase75/lanes/<name>.md`: per ledger row — what changed, the proof command and an output excerpt, run ids, the proposed new status cell. Propose new rows as text only; the lead assigns `DBT-P75-NNN` ids.
- No `Cargo.lock` / `pnpm-lock.yaml` change (only the dependency lane may). Prefer std, an existing dependency, or a local `extern "system"` declaration (the pattern `apps/consent-broker` and `apps/install-hardener` use). If a new dependency is truly unavoidable, stop and report.
- Never `git add -A` / `git add .`; never glob-run `scripts/*.py`; never force-push; never touch `release/dependency-*` by hand; never hand-edit or hand-merge `MANIFEST.sha256` or `.github/MANIFEST.sha256`.

## Traps — each one cost a past phase
- `#[cfg(windows)]` code: an "unused" import may be live, including via `use super::*` elsewhere. A grep for `use x` does not prove a dependency unused (proc-macros emit `::windows_core::`).
- Text-token gates guard symbols the compiler cannot see: renaming or moving code can turn `scripts/static_validate.py`, `scripts/*-audit.py` or `scripts/ps_marker_scan.py` red. Run them.
- `services/maintenance-service/src/router/dispatch.rs` is at its 258-line ceiling (0 headroom); `main.rs` and `router.rs` throw at ≥220 lines (now 161 and 122); every `router/*.rs` ≤ 258.
- Evidence files keep CRLF on purpose (`.gitattributes`). `[ProgramData]` is not a Burn variable. `PendingFileRenameOperations` on windows-2025 reads `*1\??\<path>`.
- api.github.com is intermittent — retry before concluding a failure.
- Some audit scripts rewrite tracked files (e.g. `SBOM.cdx.json`). After running gates, run `git status` and restore ONLY the files a script touched that you did not mean to change (`git checkout -- <path>`).

## Local proof before EVERY push (run from phase21-workspace/)
```
cargo fmt --all -- --check
cargo clippy -p <each touched crate> --all-targets --locked -- -D warnings
cargo clippy -p <crate> --target x86_64-pc-windows-msvc -- -D warnings   # see note
cargo test -p <each touched crate> --locked      # and `cargo test --workspace --locked` once before the PR
python3 scripts/static_validate.py
python3 scripts/test_gate_readers.py
python3 scripts/ps_marker_scan.py                # read its UNMEASURED lines too
python3 scripts/<every *-audit.py that names a file you touched>   # find them: grep -l '<path fragment>' scripts/*.py
```
Notes:
- On `main`, workspace-wide clippy on macOS already fails in `crates/driver-backup` and `crates/performance-telemetry/src/macos_impl.rs`. Lane `mac-clippy` owns that; other lanes must not fix it, but must add no new findings.
- The Windows-target clippy works only for crates whose dependency graph has no C build script. It fails on anything that pulls `libsqlite3-sys` (i.e. `persistence` and most crates above it), `llama-cpp-sys-2` (`intelligence-core`) or `system-repair`'s DISM `build.rs` — for those, the Windows CI job is the compile verdict; say so in your report rather than implying it passed.

## Seal — every commit that changes a file under phase21-workspace/ or .github/
1. `python3 scripts/source_seal.py --json` — every failing path it lists must be a file you changed. If not, stop and report.
2. `git add <explicit paths>`
3. `python3 scripts/regenerate-source-manifest.py`
4. `git add MANIFEST.sha256` (from inside phase21-workspace/), and `git add ../.github/MANIFEST.sha256` if the commit touches `.github/` (P75 seal-root: the repository root has its own manifest)
5. `python3 scripts/source_seal.py` must print OK. Then commit.
Since P75 seal-root, `.github/**` is sealed too, by `.github/MANIFEST.sha256`: the same steps apply to a commit that changes only a workflow (run them from phase21-workspace/).

## PR and CI
- Push `lane/<name>` and open ONE PR to `main` with `gh pr create`. Title: `<type>(<area>): <what> (P75, <ledger ids>)`. Body: what, why, proof; end it with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- `ci.yml` runs on `pull_request`: a windows job (~70–90 min) plus deny-check. Find the run with `gh run list --branch lane/<name> --workflow CI --json databaseId,headSha,status,conclusion`. Wait on it with a background `gh run watch <id> --exit-status` (Bash with run_in_background) — never poll in a loop, never foreground-sleep.
- Green is claimed only for a run whose `headSha` equals your branch head. A new push cancels the in-flight run for that ref, so batch your commits before pushing.
- Windows installer and service behaviour is proven on the runner via `windows-installer.yml` `workflow_dispatch` (job `bundle-log-acl-probe`, ~45 min) — never asserted.
- CI red: `gh run view <id> --log-failed`, diagnose, fix, push. If the same fix fails twice, stop and report the diagnosis.
- DO NOT MERGE. The lead merges one PR at a time.

## Final message (≤600 words)
PR url; branch head sha; CI run id and conclusion at that sha (and probe run ids if any); the local proof commands with their result lines; ledger rows moved, with proof; proposed new rows; what was not done and why; anything you measured that contradicts your brief (the measurement wins — say so).
