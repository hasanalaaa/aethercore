# Signer / Publisher Trust Specification

`PlatformVerifier` now returns typed `SignatureVerification` evidence rather than success/failure alone:

- signature validity
- signer subject
- signer thumbprint/identity
- chain status
- test-signed state

The acquisition engine validates actual signer evidence against provider policy. A cryptographically valid signature from an unexpected publisher returns `UnexpectedPublisher`; missing signer identity cannot be promoted to trusted.

Explicit provider-approved signer identities, when configured, take precedence over display-name matching. The subject fallback is normalized exact matching, not substring matching. Test-signed evidence is rejected.

## Native qualification boundary

The current WinVerifyTrust source implementation proves signature validity but intentionally reports `ValidIdentityNotExtracted` until Windows-native signer extraction is implemented and qualified. Consequently the hardened publisher gate fails closed for DirectTrusted acquisition rather than inventing identity evidence.

`DirectTrusted` production installation remains disabled until Windows qualification proves signer identity extraction, provider certificate policy, staging security, privileged revalidation, package applicability, and provider installer contracts.
