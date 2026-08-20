# Driver Override Scoping Specification

`IgnoreExactVersion` is explicitly scoped to:

- device identity
- authority/provider identity
- exact candidate version
- policy type

Suppression requires the stored provider identity to be present and equal to the candidate provider. A blank provider fails closed. The same version from another official authority is not hidden, and a newer version remains visible.

A future “ignore this version across all sources” behavior, if ever added, must be a distinct explicit policy; it is not silently inferred from `IgnoreExactVersion`.
