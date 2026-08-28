# Offline Distribution

`scripts/phase35-release.py bundle` creates a deterministic ZIP with fixed timestamps, sorted members and no network dependency. It contains only the installer/package, release manifest/signature, SBOM and provenance. The verifier rejects traversal, absolute/drive/UNC paths, duplicate names, `.DS_Store`, private-key extensions, developer caches and bounded-size violations.
