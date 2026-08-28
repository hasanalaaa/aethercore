# Rollback Policy

An ordinary update cannot target an older or equal version. Emergency rollback is a distinct detached-signed `aethercore.release.rollback-authorization.v1` containing product, current version, explicitly authorized target, channel, reason code, expiry and signer key ID. The target package digest and manifest are reverified before rollback. A generic `--force` or UI action cannot bypass these checks.
