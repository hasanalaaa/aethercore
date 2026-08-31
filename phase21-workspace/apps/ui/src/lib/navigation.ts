import type { MessageKey } from './i18n';

/** Stable semantic route identifiers. Display labels live exclusively in the message catalogs. */
export type PageId = 'overview' | 'deepScan' | 'drivers' | 'repair' | 'cleanup' | 'startup' | 'performance' | 'hardware' | 'crash' | 'activity' | 'fleet';
export type IconName = 'overview' | 'scan' | 'drivers' | 'repair' | 'cleanup' | 'startup' | 'performance' | 'hardware' | 'crash' | 'activity' | 'search' | 'language' | 'theme' | 'check' | 'warning' | 'service' | 'fleet';
export type NavigationGroup = 'observe' | 'act' | 'history';
export type NavigationAvailability = 'available' | 'online';

export type NavigationItem = {
  id: PageId;
  icon: IconName;
  labelKey: MessageKey;
  descriptionKey: MessageKey;
  shortcut: string;
  group: NavigationGroup;
  availability: NavigationAvailability;
};

export const NAVIGATION: readonly NavigationItem[] = [
  { id: 'overview', icon: 'overview', labelKey: 'nav.overview', descriptionKey: 'nav.overviewDescription', shortcut: 'Ctrl+Shift+1', group: 'observe', availability: 'available' },
  { id: 'deepScan', icon: 'scan', labelKey: 'nav.deepScan', descriptionKey: 'nav.deepScanDescription', shortcut: 'Ctrl+Shift+2', group: 'observe', availability: 'available' },
  { id: 'drivers', icon: 'drivers', labelKey: 'nav.drivers', descriptionKey: 'nav.driversDescription', shortcut: 'Ctrl+Shift+3', group: 'act', availability: 'available' },
  { id: 'repair', icon: 'repair', labelKey: 'nav.repair', descriptionKey: 'nav.repairDescription', shortcut: 'Ctrl+Shift+4', group: 'act', availability: 'available' },
  { id: 'cleanup', icon: 'cleanup', labelKey: 'nav.cleanup', descriptionKey: 'nav.cleanupDescription', shortcut: 'Ctrl+Shift+5', group: 'act', availability: 'available' },
  { id: 'startup', icon: 'startup', labelKey: 'nav.startup', descriptionKey: 'nav.startupDescription', shortcut: 'Ctrl+Shift+6', group: 'act', availability: 'available' },
  { id: 'performance', icon: 'performance', labelKey: 'nav.performance', descriptionKey: 'nav.performanceDescription', shortcut: 'Ctrl+Shift+7', group: 'observe', availability: 'available' },
  { id: 'hardware', icon: 'hardware', labelKey: 'nav.hardware', descriptionKey: 'nav.hardwareDescription', shortcut: 'Ctrl+Shift+8', group: 'observe', availability: 'available' },
  { id: 'crash', icon: 'crash', labelKey: 'nav.crash', descriptionKey: 'nav.crashDescription', shortcut: 'Ctrl+Shift+9', group: 'observe', availability: 'available' },
  { id: 'activity', icon: 'activity', labelKey: 'nav.activity', descriptionKey: 'nav.activityDescription', shortcut: 'Ctrl+Shift+0', group: 'history', availability: 'available' },
  { id: 'fleet', icon: 'fleet', labelKey: 'nav.fleet', descriptionKey: 'nav.fleetDescription', shortcut: 'Ctrl+Shift+F', group: 'history', availability: 'online' },
] as const;

export const NAVIGATION_GROUPS: readonly { id: NavigationGroup; labelKey: MessageKey }[] = [
  { id: 'observe', labelKey: 'nav.group.observe' },
  { id: 'act', labelKey: 'nav.group.act' },
  { id: 'history', labelKey: 'nav.group.history' },
] as const;
