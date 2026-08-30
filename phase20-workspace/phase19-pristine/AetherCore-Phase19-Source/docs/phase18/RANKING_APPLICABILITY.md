# Authority Ranking and Applicability

Candidate ordering is deterministic and typed. It considers trust, applicability, machine context, device specificity, machine specificity, and only then provider-aware numeric version evidence. Dotted numeric versions are compared without assuming SemVer; nonnumeric/vendor-specific formats become `VersionOrdering::Unknown`.

Applicability states are `Exact`, `Compatible`, `Contextual`, `Incompatible`, and `Unknown`. Incompatible or unknown package candidates cannot become executable recommendations. Firmware is always `FirmwareProtected` in this phase.

Context policy supports machine-specific OEM authority, Windows Update, component-vendor reference authority, and official utilities without encoding “OEM always wins” or “newer always wins.” On known NVIDIA/AMD/Intel display devices the official utility can be the recommended authority while a WUA offer remains an alternative. An unknown display vendor can still use an exact trusted WUA offer; Display class alone is no longer a blanket prohibition.

Recommendation reasons are closed reason codes including OEM specificity, Windows applicability, vendor utility requirement, exact/compatible match, missing/problem priority, newer-but-lower-authority, firmware review, trust rejection, incomplete coverage, and user defer/ignore state.
## Coverage truth

Windows Update `WU_E_NO_CONNECTION` is normalized to `Offline`; provider warnings become `Partial`; hard provider errors become `ProviderUnavailable`. Only `CompleteForKnownProviders` is eligible for an “up to date from checked sources” result. This prevents network failure from masquerading as driver health.
