# Distributing `aetherctl` — the terminal-first channel

Phase 37 Stage 3. This document is the decision and its justification, per
platform, plus the contract a script can rely on.

## The requirement

`aetherctl` must be installable and usable straight from a terminal:

- installing the CLI alone must not require the desktop app or any GUI
- it must work headless, with no interactive session
- `aetherctl --help` must be a genuinely good first experience
- exit codes and JSON output must be stable enough to script against, and documented

## The channel, per platform, and why

### Windows — a plain signed archive is the channel; winget is discoverability

**Primary: portable zip.** `scripts/build-cli-archive.ps1` produces
`aetherctl-<version>-windows-<arch>.zip` plus `SHA256SUMS.txt`. Install is one
command:

```powershell
Expand-Archive aetherctl-<version>-windows-<arch>.zip -DestinationPath $env:ProgramFiles\AetherCLI -Force
```

Then put that folder on `PATH`, or call the executable by full path.

This is the right shape because of what the CLI actually is: a single executable
that installs no service, registers nothing, writes nothing outside its own
folder and needs no elevation. There is no install transaction to perform, so
there is nothing for an installer to do. Uninstall is `Remove-Item` on the
folder — which is also why the Stage 2 "leave no trace" work does not apply to
this channel: there is no trace to leave.

Authoring a second MSI to carry one executable would add an ARP entry, an
install/repair/uninstall transaction and a rollback path, all of which would
have to be qualified, in exchange for nothing the archive does not already do.
**We did not add a package-manager dependency where a plain archive does it.**

**Secondary: a winget manifest** (`packaging/winget/`), for the person who
expects `winget install AetherCore.aetherctl` to work. It is authored and it is
**not publishable yet**, because a winget manifest is a pointer to a hosted
archive plus a SHA-256, and both of its prerequisites are human-gated here: a
production download endpoint, and an Authenticode certificate (winget's
`portable` type puts the binary on the user's PATH; SmartScreen will treat an
unsigned one accordingly). The four `PENDING_` placeholders are filled by
release automation from `out/cli/SHA256SUMS.txt`. See
`packaging/winget/README.md`.

**Not chosen: Chocolatey / Scoop.** Both would be a third packaging surface to
keep in sync for the same zip, and neither is present by default on Windows.
winget is, from Windows 11 22621 — which is already the product's floor
(`Product.wxs` requires build 22621).

### macOS and Linux — a tarball, and the unit files the CLI already prints

Same reasoning, same artifact shape: `aetherctl-<version>-<os>-<arch>.tar.gz`
plus a SHA-256, expanded anywhere on `PATH`. `aetherctl` already ships the
service unit files for both platforms and prints them on demand:

```
aetherctl service units --print
```

which emits the launchd plist and the systemd unit. **Nothing auto-installs
them**; installation is an explicit administrator act, by design
(`apps/aetherctl/src/units.rs`).

**Not chosen: Homebrew tap or a distro package.** A tap needs a public
repository; a `.deb`/`.rpm` needs a signed repository and a maintainer per
distribution. Both are the same human-gated hosting-and-signing blocker as
winget, with more surface. When that endpoint exists, a tap is a thin wrapper
over the same tarball and can be added without changing anything here.

**Honest limit:** the unix CLI is not qualified in this phase. It compiles and
its offline verbs run, but no Linux or macOS host was exercised as a gate.
`QD-028-002` already records the systemd unit as verification-deferred.

## What the CLI alone can do

Installed on its own, with no maintenance service present:

| works offline, no service | needs the service |
|---|---|
| `about`, `version`, `capabilities`, `engine-source` | `doctor` |
| `self-check [--load-model]` | `perf start\|stop\|snapshot\|report` |
| `service detect`, `service units --print` | `optimize plan\|start\|status` |
| `sec audit`, `sec report` | `timeline page\|patterns` |
| `compliance summary`, `compliance verify` | `care status\|start\|cancel` |
| `release inspect`, `release verify` | `insights list\|explain\|dismiss` |
| `update offline verify <bundle.zip>` | `scan start\|cancel\|status\|history` |

A service verb without a service is not a hang and not a crash: it exits **3**
with a typed envelope.

`self-check` reports the embedded language model as absent in a CLI-only
install. That is correct — the 1.07 GB model ships with the full product, not
with the command surface — and it is reported rather than hidden.

## The scripting contract

### Exit codes

| code | meaning |
|---|---|
| 0 | ok |
| 2 | usage error (the command line did not parse) |
| 3 | maintenance service unreachable |
| 4 | request timed out |
| 5 | rejected by the service |
| 6 | consent required |
| 7 | capability unavailable on this platform |
| 8 | local I/O error |
| 130 | interrupted (SIGINT) |

The registry lives in `apps/aetherctl/src/exit.rs` and is unit-tested against
this table (`exit_code_registry_matches_docs_phase28`). It is also printed by
`aetherctl --help`, so it is readable without this document.

### JSON

`--output json` prints exactly one object per invocation:

```json
{"schema":"aethercore.aetherctl.v1","command":"<verb>","ok":true,"data":{ }}
{"schema":"aethercore.aetherctl.v1","command":"<verb>","ok":false,
 "error":{"kind":"...","message_key":"...","detail":"..."}}
```

The stable surface is: the `schema` string, `command`, `ok`, the closed set of
`error.kind` values, the `message_key` vocabulary, and the exit codes. Rendered
text is **not** stable and must not be parsed.

**One exception, stated rather than hidden:** a command line that fails to
*parse* is reported as text on stderr with exit 2, because `--output` is itself
part of the line being parsed. Everything past parsing is JSON when JSON was
asked for.
