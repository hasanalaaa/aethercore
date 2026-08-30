# Trust, Provenance, and Secure Acquisition

`driver-acquisition` implements a user-scope HTTPS download boundary for future `DirectTrusted` providers. Requests are provider-bound and require an expected SHA-256 digest, provider network policy, byte bounds, and expected publisher metadata. Redirects are disabled in the HTTP client and followed manually only after every hop is checked against the provider’s authority/distribution host policy. Redirect count and download size are bounded.

Downloads use a create-new partial file, stream bytes while hashing, support cancellation before privileged mutation, fsync, verify expected size/hash, call the existing narrow platform signature verifier, and atomically freeze the staged file. Staging-root validation rejects non-absolute roots and immediate symlink/reparse ambiguity at the portable boundary. Native NTFS ACL/hardlink/reparse proof remains explicit qualification debt.

HTTPS alone is never treated as package trust. Package trust states distinguish Windows-managed, trusted signature, trusted signature+digest, unexpected publisher, invalid, unsigned, unverifiable, test-signed, and not-applicable utility flows.
