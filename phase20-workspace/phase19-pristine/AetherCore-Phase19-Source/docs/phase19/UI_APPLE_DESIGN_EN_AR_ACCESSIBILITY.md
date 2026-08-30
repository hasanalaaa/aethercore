# Repair UI — Apple Design, EN/AR & Accessibility Evidence

The existing Repair surface is upgraded in place to **Windows Health**. It does not create another standalone utility.

## Truth hierarchy

The screen shows domain health, typed diagnosis roles, recovery readiness, technical evidence disclosure and a Repair Recommended section only when the service graph contains a currently supported executable action. Healthy completed checks show that no repairable issue was detected among the performed checks; unknown probes remain unknown.

The review sheet shows the executable nodes from the same service-supplied graph, recovery state, reboot boundary and graph digest before UAC. A reboot barrier suppresses ordinary execution rather than allowing a button that the backend would reject.

## Apple Design contract

- `Pressable` / `fluidPress` preserve pointer-down feedback.
- Existing fluid dialogs remain interruptible rather than transition-locked.
- Progress derives from service task state; no time-driven fake repair percentage is introduced.
- translucent material hierarchy is retained for health/review surfaces;
- `prefers-reduced-motion`, `prefers-reduced-transparency` and `prefers-contrast` accommodations remain in the design system;
- RTL uses logical layout and technical data remains isolated through `TechnicalText`.

## Localization and accessibility

English and Arabic catalogs have exact key parity. All new repair domains, diagnoses, actions, safety states, recovery/reboot text and technical-detail labels are localized. The main state region uses restrained `aria-live`, details are keyboard-native disclosures, and review controls remain keyboard-usable.

Narrator/WebView2 behavior, real Windows reduced-transparency behavior and 200% text runtime layout remain explicit qualification debt rather than source-level native PASS.
