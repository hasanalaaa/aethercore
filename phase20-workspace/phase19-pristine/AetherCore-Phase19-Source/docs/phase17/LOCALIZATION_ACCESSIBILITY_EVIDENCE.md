# Phase 17 Localization, UI & Accessibility Evidence

## Localization

The executed static audit reports exact EN/AR catalog parity for Phase 17: **176 Phase 17 keys**, with no missing English keys and no missing Arabic keys. Deep Scan uses complete localized messages rather than concatenating sentence fragments. Collector states, reboot semantics, reversibility, progressive-finding messages and persistence-continuity limitations have explicit localized keys.

Technical identifiers such as device IDs, rule IDs and evidence values remain technical content and are rendered separately from primary prose so mixed LTR/RTL content can be isolated appropriately.

## Interaction and motion

The production Deep Scan route uses AetherCore's established `Pressable`, `ProgressBar`, `MaterialSurface` and `TechnicalText` primitives. This preserves pointer-down feedback, spring-retargeted progress, reduced-motion handling, reduced-transparency/contrast integration and the existing material hierarchy. No timer-driven or CSS-keyframe fake scan progression is introduced.

## Accessibility source evidence

- Primary scan/cancel actions are semantic controls through the existing Pressable primitive.
- Finding groups and technical details use semantic article/details structures.
- Filter controls expose pressed state rather than relying on color alone.
- Severity is communicated with text, not color alone.
- Progressive scan discovery uses restrained polite live status for finding-count changes instead of announcing each progress tick.
- Production navigation remains keyboard-accessible and the Deep Scan page is part of the same focus/navigation system.

## Qualification boundary

Catalog parity and source invariants are executed/static evidence. Installed Svelte checks, browser rendering, real screen-reader runs, 200% text-scaling verification, long-Arabic visual clipping tests and Windows high-contrast/reduced-motion runtime behavior are `NOT_EXECUTED` on this host and remain qualification debt. No runtime accessibility PASS is claimed.
