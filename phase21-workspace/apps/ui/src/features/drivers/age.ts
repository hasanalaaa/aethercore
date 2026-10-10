import type { DriverDevice, InstalledDriver } from '../../lib/contracts';

const YEAR_MS = 365.25 * 24 * 60 * 60 * 1000;

/**
 * The registry's DriverDate is `m-d-yyyy`; the wire fixture and Windows Update use `yyyy-mm-dd`.
 * Anything else, or a day that does not exist, is unknown: an age nobody measured is not zero.
 */
export function parseDriverDate(text: string): number | undefined {
  const value = text.trim();
  const iso = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(value);
  const us = /^(\d{1,2})[-/](\d{1,2})[-/](\d{4})$/.exec(value);
  const parts = iso ? [+iso[1], +iso[2], +iso[3]] : us ? [+us[3], +us[1], +us[2]] : undefined;
  if (!parts) return undefined;
  const [year, month, day] = parts;
  const ms = Date.UTC(year, month - 1, day);
  const back = new Date(ms);
  return back.getUTCFullYear() === year && back.getUTCMonth() === month - 1 && back.getUTCDate() === day ? ms : undefined;
}

/**
 * A package a vendor installed is stored as `oemNN.inf`; Windows' own drivers keep their names
 * (`usb.inf`, `intelpmt.inf`) and carry a 2006 date, so an age on them would only alarm.
 */
export function isVendorDriver(driver: InstalledDriver | null | undefined): boolean {
  return /^oem\d+\.inf$/i.test(driver?.infPath ?? '');
}

export function vendorDriverAgeYears(device: DriverDevice, nowMs: number): number | undefined {
  if (!isVendorDriver(device.driver)) return undefined;
  const dated = parseDriverDate(device.driver?.date ?? '');
  return dated === undefined ? undefined : Math.max(0, (nowMs - dated) / YEAR_MS);
}
