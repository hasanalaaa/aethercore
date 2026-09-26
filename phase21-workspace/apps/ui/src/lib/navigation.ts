import type { MessageKey } from './i18n';

/**
 * Stable semantic route identifiers. Display labels live exclusively in the message catalogs.
 *
 * P58: `'assistant'` was removed. The drawer is an overlay on top of whatever
 * screen is active, never a destination — `DIRECTION.md` rejected a dedicated
 * screen and `NavigationRail.svelte` says it "must not read as one". Listing it
 * in a union whose contract is "routes" invites the next reader to give it a nav
 * entry. `IconName` keeps it: the drawer does have an icon.
 */
export type PageId = 'overview' | 'deepScan' | 'drivers' | 'repair' | 'cleanup' | 'startup' | 'performance' | 'hardware' | 'crash' | 'activity' | 'fleet' | 'settings';
export type IconName = 'overview' | 'scan' | 'drivers' | 'repair' | 'cleanup' | 'startup' | 'performance' | 'hardware' | 'crash' | 'activity' | 'search' | 'language' | 'theme' | 'check' | 'warning' | 'service' | 'fleet' | 'settings' | 'assistant';
export type NavigationGroup = 'observe' | 'act' | 'history' | 'system';
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
  // §51.3 and DBT-P50-003. Four sections on the Overview were configuration
  // wearing status styling — the update channel, the support bundle, the build
  // identity and the capability matrix. This is the destination they needed and
  // the app did not have.
  { id: 'settings', icon: 'settings', labelKey: 'nav.settings', descriptionKey: 'nav.settingsDescription', shortcut: 'Ctrl+Shift+S', group: 'system', availability: 'available' },
] as const;

export const NAVIGATION_GROUPS: readonly { id: NavigationGroup; labelKey: MessageKey }[] = [
  { id: 'observe', labelKey: 'nav.group.observe' },
  { id: 'act', labelKey: 'nav.group.act' },
  { id: 'history', labelKey: 'nav.group.history' },
  { id: 'system', labelKey: 'nav.group.system' },
] as const;

/** The keyboard fields a navigation shortcut is decided from. */
export type ShortcutKeys = Pick<KeyboardEvent, 'ctrlKey' | 'shiftKey' | 'altKey' | 'metaKey' | 'key' | 'code'>;

/**
 * The page a `Ctrl+Shift+<key>` press navigates to, if any. Read from `code`, the physical key:
 * with Shift held `key` is "!" for Digit1 on a US layout, and a letter of another script on an
 * Arabic one, so matching `key` made every digit shortcut dead (P75). Ctrl+Alt is AltGr on
 * Windows and is never navigation.
 */
export function navigationShortcut(event: ShortcutKeys): PageId | undefined {
  if (!event.ctrlKey || !event.shiftKey || event.altKey || event.metaKey) return undefined;
  const physical = /^(?:Digit|Key)([0-9A-Z])$/.exec(event.code)?.[1];
  if (!physical) return undefined;
  return NAVIGATION.find((candidate) => candidate.shortcut === `Ctrl+Shift+${physical}`)?.id;
}

/** `Ctrl+K` opens the command palette, `Ctrl+/` the assistant. Physical keys, for the same
 * reason as `navigationShortcut`: on an Arabic layout K types "ن" and / types "ظ". */
export function commandShortcut(event: ShortcutKeys): 'palette' | 'assistant' | undefined {
  if (!(event.ctrlKey || event.metaKey) || event.shiftKey || event.altKey) return undefined;
  if (event.code === 'KeyK') return 'palette';
  if (event.code === 'Slash') return 'assistant';
  return undefined;
}
