# Drivers UI and EN/AR Evidence

The existing Drivers surface is retained. Phase 18.1 changes semantics rather than rebuilding the dashboard.

Each device can now present four independent facts:

- current driver state/version
- update availability
- official management authority
- authority coverage

New English/Arabic keys cover update unknown, no confirmed update, provider unavailable, partial/manual coverage, management availability, update-evidence state, and authority lists. Exact catalog parity is checked by the Phase 18/18.1 audits.

Apple-design behavior is preserved: `fluidPress`/pointer-down tactile response remains wired, reduced motion/transparency/contrast accommodations remain in the design system, and the existing RTL/technical-text handling is retained. Truthful wording takes precedence over decorative motion.
