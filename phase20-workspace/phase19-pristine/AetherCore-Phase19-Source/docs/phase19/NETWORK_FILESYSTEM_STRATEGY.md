# Network, Proxy, DNS & Filesystem Strategy

## Network family

Phase 19 defines independent Network, DNS and Proxy facts/diagnoses and safety-ranked actions. DNS repair is only graph-eligible from DNS-specific evidence; generic offline/network failure does not create a DNS or Winsock diagnosis. Managed or unexpected proxy configuration is review-first and cannot be silently removed.

The runtime deliberately does not ship a broad network reset, third-party DNS substitution or generic Winsock reset without a qualified native evidence provider. Additional structured DNS/DHCP/proxy/Winsock provider depth is recorded in Product Capability Debt rather than filled with internet tweak scripts.

## Filesystem

The implemented read path is `CHKDSK /scan` against the system volume. Nonzero filesystem scan results create filesystem attention only; they do not assert SSD/HDD physical failure. Physical storage reliability remains a separate domain.

Offline filesystem repair is modeled as Level 3, recovery-protected, reboot-aware and non-automatic. Phase 19 does not automatically use `/f`, `/r`, `/spotfix`, format, partition mutation or raw-disk writes.
