# ADR 0011 — Principal-bound operations, consent intents, and exact cleanup identity

**Status:** Accepted — Phase 9.

## Context

The Phase 8 named pipe is local-only and the elevated broker is path/elevation checked, but a machine with multiple authenticated interactive users needs a stronger invariant: service state, discovery evidence, plans, authorization, history, and mutations must never become a confused-deputy capability across Windows logon principals. The previous challenge/grant authorization vocabulary also created an avoidable reusable authorization window. Cleanup already used strong final-path/reparse protections but still needed exact file-object identity to reject a same-path replacement between scan and mutation.

## Decision

1. Derive `PrincipalContext` inside the service from the connected named-pipe client. The service impersonates that exact pipe client, opens the current thread token, reads user SID / token `AuthenticationId` / elevation from the impersonated token, reads Terminal Services session ID from the pipe, and reverts before parsing/dispatching the request. PID is not trusted for ownership; it is used only to resolve executable image identity. The authorization binding key is a digest of SID + `AuthenticationId` + session ID. The UI never supplies an owner key.
2. Persist `owner_principal_key` on immutable plans and scope all plan/history/status APIs to the connected principal. Driver, repair, cleanup, startup, and diagnostic in-memory/readback state is owner-scoped as well.
3. Replace command-line challenge/grant authorization with short-lived service-minted consent intents. Elevation receives only `intent_id`. The broker must be elevated, match the installed broker image, and remain the same logon principal. It retrieves the plan summary from the service.
4. Permit an intent to be approved once and presented once. Consume one approved intent atomically in the SQLite transaction that advances the owned plan from `AwaitingAuthorization` to `Preflight`.
5. Freeze cleanup target volume serial + `FILE_ID_INFO` 128-bit file identifier from the opened target handle. Revalidate identity before mutation and delete through that validated handle.
6. Retire prototype/demo transition payloads and challenge/grant payloads from protocol v7 and production source.
7. Make release dependency approval fail-closed: lockfiles and their generating manifests must match committed hash baselines; CI may verify but never mint an approved baseline.

## Consequences

- A plan and its authorization cannot intentionally cross user/logon/session ownership.
- UAC approval is a one-shot transition capability, not a timed generic grant.
- Replacing a cleanup target with a different file object at the same path is rejected.
- Protocol version is 7 and old prototype authorization/demo payloads are not supported.
- A source archive without the approved lock/freeze evidence is buildable only after an explicit trusted dependency-freeze step and is not a release candidate.
