# P36 diagnostic probes — NOT product source, NOT shipped

Six named-pipe / raw-wire IPC probes written during Phase 36 tranche 1 to
qualify the Windows ARM64 named-pipe transport. They lived in
`crates/ipc/examples/` (where `cargo build --examples` would compile them into
the product workspace); they were relocated here so they cannot enter a build.

They are Windows-only and are run by hand on the qualification VM:

```
rustc --edition 2021 -L <target>/release/deps tools/p36-probes/ipc_probe.rs
```

They depend on the diagnostic-only re-export `aethercore_ipc::probe`
(`crates/ipc/src/lib.rs`, `#[cfg(windows)]`), which widened the frame codec from
`pub(crate)` to `pub`. See `docs/phase36/DRIFT_LEDGER.md` (DBT-P36-002) — that
widening is tracked debt and reverts with these probes.
