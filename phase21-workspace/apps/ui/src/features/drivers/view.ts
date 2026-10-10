import type { DriverDevice, DriverFilter, DriverHub } from '../../lib/contracts';
import { isVendorDriver, parseDriverDate } from './age';

/** The device list for a filter and a search; `Vendor` is third-party drivers, oldest first. */
export function filterDevices(devices: readonly DriverDevice[], filter: DriverFilter, search: string, _nowMs: number): DriverDevice[] {
  const query = search.trim().toLowerCase();
  const matched = devices.filter((device) => {
    const filterMatch = filter === 'All'
      || (filter === 'Updates' && device.candidates.some((candidate) => candidate.recommendationState === 'Recommended' || candidate.recommendationState === 'Optional'))
      || (filter === 'Problems' && device.hasProblem)
      || (filter === 'Missing' && device.missingDriver)
      || (filter === 'Display' && (device.className.toLowerCase() === 'display' || device.displayManaged))
      || (filter === 'Vendor' && isVendorDriver(device.driver))
      || (filter === 'Managed' && (device.displayManaged || device.managementAuthorities.length > 0));
    if (!filterMatch) return false;
    if (!query) return true;
    return [device.displayName, device.manufacturer, device.className, device.instanceId, ...device.hardwareIds]
      .some((value) => value.toLowerCase().includes(query));
  });
  if (filter !== 'Vendor') return matched;
  const dated = (device: DriverDevice) => parseDriverDate(device.driver?.date ?? '') ?? Number.POSITIVE_INFINITY;
  return [...matched].sort((a, b) => dated(a) - dated(b));
}

/** The inventory is local and read-only, so it loads when the page opens: once, when connected, if nothing was scanned. */
export function shouldAutoScan(hubState: string, connected: boolean, alreadyStarted: boolean): boolean {
  return hubState === 'Idle' && connected && !alreadyStarted;
}

/** Set when the list is only what Windows already had: the update search did not run. */
export function searchNotice(hub: Pick<DriverHub, 'state' | 'searchScope' | 'windowsLastOnlineSearch'>): { date: string } | null {
  return hub.state === 'Ready' && hub.searchScope === 'LocalCacheOnly' ? { date: hub.windowsLastOnlineSearch } : null;
}
