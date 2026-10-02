# Unix live-event acceptance follow-up (2026-10-02)

`DBT-P75-002` identified an actual transport defect: Unix sessions installed an owner-scoped
subscription but never consumed it after Hello/replay. A live socket test against the real
ServiceContext failed with a read timeout when the bus published an owned event without a client
request. The Windows host already forwarded these events.

The Unix session now drains its existing subscription on a named worker. Events and RPC responses
use one synchronized writer, preventing length/body interleaving across cloned streams. Replay reset
and subscriber lag request current typed hydration through the existing streaming helper. A five-second
write bound prevents an unresponsive reader holding the session writer indefinitely; idle reads are
not expired by that setting. Session RAII cancels outstanding work, shuts down the socket and joins
the event pump, dropping the subscription on Goodbye, disconnect and early exits. Socket permissions,
owner binding, Windows pipes, protocol tags and product egress are unchanged.

The newly exercised legacy native tests initially failed because they interpreted the next frame as
an RPC response, although live events can precede it. Their exchange helper now accepts typed event
frames while waiting for the exact request-id response under one 30-second read deadline. The
security fixture also supplies its isolated data directory explicitly. No authorization assertion
was removed. Unused imports and one unnecessary borrow became visible under feature-enabled Clippy
and were corrected without suppression.

## Local evidence

On macOS with the pinned Rust 1.97.1 toolchain:

- `cargo test --locked -p aethercore-ipc -p aethercore-maintenance-service`: 99 passed, zero failed.
  This includes nine adversarial Unix transport tests, live delivery, foreign-owner exclusion,
  replay reset/current hydration and subscription release.
- `cargo test --locked -p aethercore-maintenance-service --features unix-ipc
  --test phase27_unix_ipc --test phase39_ipc_authorization`: eight passed, zero failed.
  Actual service-process round trips, stale socket restart, asynchronous embedded-model loading,
  owner-only audit targets and foreign journal denial were exercised. The existing model artifact
  was reused via a local hard link; nothing was downloaded and no product data was purged.
- A real socket test proves a write deadline rejects an unread four-MiB frame while preserving an
  idle reader beyond that short test deadline.
- The full affected-target Clippy checks pass with warnings denied, including `unix-ipc` test paths.
- Formatting, 351 static checks and the required-aggregate CI self-tests pass.

Root raw logs: `aethercore-unix-live-red.log`, `aethercore-unix-live-full.log`,
`aethercore-unix-feature-tests.log` (the first legacy failure),
`aethercore-unix-feature-green.log`, `aethercore-unix-clippy.log`,
`aethercore-unix-feature-clippy-green.log`, `aethercore-unix-static.log`,
`aethercore-unix-ci-gates.log` under the execution session's `/tmp` evidence directory.
These logs are local receipts, not portable artifact claims.

CI adds actual Ubuntu Unix adversarial/service/authorization tests and feature-enabled Clippy to
its existing Linux provider gate. The macOS embedded-model test remains macOS-specific. Linux CI
at the final source head is pending in the PR; Windows native CI must also pass before integration.
No Windows installed, Narrator, hardware matrix or signed-RC qualification is claimed by this fix.

## First Linux CI and correction

At source `6dbf1d2`, CI run `36993848471` passed all nine native transport tests and both
real-ServiceContext live/reset tests. The older service-process test then failed because it
hard-coded `macos` for both capability and engine-source responses while Ubuntu correctly
returned `linux`. Both assertions now compare against `std::env::consts::OS`; the native
availability and authorization assertions remain unchanged. This is a platform-specific test
expectation correction, not a product fallback or removed assertion. Raw completed-job log:
`/tmp/aethercore-unix-linux-job.txt`. Final-head Linux CI must pass after this correction.
