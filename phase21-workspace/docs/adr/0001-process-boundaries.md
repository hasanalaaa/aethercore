# ADR-0001: Three-process privilege architecture

**Status:** Accepted

The visible desktop remains non-elevated. A per-machine Windows service owns system mutations. A small UAC consent broker is launched only for an exact reviewed plan. This reduces the privileged attack surface of the WebView-based desktop shell.
