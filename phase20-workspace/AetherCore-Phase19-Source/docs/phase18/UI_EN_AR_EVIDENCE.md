# Driver UI, Interaction, Accessibility, and EN/AR Evidence

The existing Drivers page was upgraded rather than forked. It now exposes recommended count, device attention, vendor-managed count, source/authority badges, authority coverage, selection tools, recommended/alternative candidates, installation mode, exact source details, and preference controls inside expanded detail surfaces.

Flow remains Scan → Review → Select → Install Selected. `Select recommended` only selects executable recommendations; firmware and vendor utility/manual paths remain review/guided actions. No fake time-based progress was introduced.

Interaction reuses the existing `fluidPress` pointer-down feedback and design motion system. Phase 18 CSS keeps reduced-motion, reduced-transparency and increased-contrast handling; keyboard focus is explicit on preference buttons. Technical values continue through `TechnicalText`/bidi isolation patterns for Arabic layouts.

The Phase 18 source audit reports exact EN/AR catalog-key parity. Native Arabic shaping, WebView2 rendering, Narrator and 50+ device performance remain qualification debt.
