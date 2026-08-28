# Release Model

`ReleaseIdentity` (`AetherCore`, semantic version, typed channel, monotonic release sequence, target platform/architecture, protocol and minimum updater versions, source identity and reproducible epoch) is canonical. Cargo workspace version is the checked-in value; Tauri, WiX, package names, manifests and CLI output must agree with it.

Release manifest bytes are canonical compact JSON with schema `aethercore.release.manifest.v1`. The signature is detached and covers the exact bytes; the manifest digest is SHA-256 of those bytes and never self-references the detached signature. Unknown fields are rejected.
