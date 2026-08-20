# Recovery Readiness & Escalation

Recovery readiness is represented explicitly as System Restore, restore-point creation, WinRE, journal recovery and driver rollback state. Presence of a directory or executable is not claimed as proof that protection is available.

A repair node can declare mandatory recovery protection. Execution validation fails closed if such a node were automatically executable while no mandatory protection is represented.

Escalation levels are typed and never loop indefinitely through repair commands:

1. retry after reboot / fresh assessment;
2. manual official repair;
3. guided repair reinstall;
4. guided WinRE recovery;
5. reset preserving files, with application-removal warning;
6. clean reinstall, with explicit data-backup requirement;
7. hardware service.

Level 4 and Level 5 actions are guidance/recovery authority only in Phase 19 and are never silently executed. Version/configuration-dependent repair reinstall and WinRE availability remain Windows-native qualification debt.
