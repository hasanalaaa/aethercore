# Exit Code Registry — aetherctl (Phase 28)

Single source of truth: `apps/aetherctl/src/exit.rs` (`pub enum ExitCode`), pinned by the
unit test `exit_code_registry_matches_docs_phase28` and enforced for tri-equality with this
document by `scripts/phase28-adversarial-audit.py` (gate p28-exitcodes-*).

| Code | Meaning |
|---|---|
| `0` | Ok — every requested operation completed; the envelope (JSON) or report (text) is valid. |
| `2` | Usage — argument parsing failed: unknown flag/command, missing or malformed value, global flag after the command. |
| `3` | Service unreachable — detection could not produce a usable endpoint for a service-backed command (Offline / stale endpoint / connect refused). Never a hang beyond `--timeout-ms`. |
| `4` | Timeout — the `--timeout-ms` deadline elapsed before the service answered. |
| `5` | Rejected by service — the maintenance service answered with a typed rejection (non-zero status code); the SERVICE's own message key is surfaced verbatim in the envelope. |
| `6` | Consent required/refused — care start needs interactive digest confirmation that was not given: non-interactive mode, JSON mode, declined prompt, wrong digest, or no plan digest available. |
| `7` | Capability not available — an embedded capability is honestly unavailable in this build/platform (e.g. `self-check --load-model` without the opt-in loader feature). |
| `8` | Local I/O / output error — manifest missing or invalid, artifact unreadable, hash mismatch (fail-closed), protocol violation, stdout failure. |
| `130` | SIGINT — default OS disposition terminates the process; POSIX shells conventionally report 130. The CLI never installs its own handler (registry-completeness variant; never constructed in-process). |

Trigger-path pinning lives in `exit.rs::tests::trigger_paths_map_one_to_one`: each code is
produced by exactly one error-kind family via `CliError::exit_code()`.
