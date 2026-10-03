import type { BatteryMeasurement, BootMeasurement, MeasurementCoverage, NetworkAdapterMeasurement, ThermalZoneMeasurement } from '../../lib/contracts';
import { formatDateTime, formatNumber, hasMessageKey, td, type Locale, type MessageKey } from '../../lib/i18n';

/** One line of a measurement list. `value` is null when the reading was not taken. */
export type MeasurementRow = {
  id: string;
  name: string;
  value: string | null;
  note: string | null;
  availability: MessageKey;
  source: string;
  observedUnixMs: number | null;
};

type Loose = Record<string, unknown>;
const isRecord = (value: unknown): value is Loose => typeof value === 'object' && value !== null;
const finite = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);
/** A reading is present only when the wire says so; the number beside a false flag is a filler. */
const read = (item: Loose, has: string, value: string): number | null => (item[has] === true && finite(item[value]) ? (item[value] as number) : null);

const AVAILABILITY: Record<number, MessageKey> = {
  1: 'measurement.availability.measured',
  2: 'measurement.availability.notMeasured',
  3: 'measurement.availability.unsupported',
  4: 'measurement.availability.denied',
  5: 'measurement.availability.failed',
};

function frame(item: Loose, locale: Locale, name: string): Pick<MeasurementRow, 'id' | 'name' | 'availability' | 'source' | 'observedUnixMs'> {
  const coverage = (isRecord(item.coverage) ? item.coverage : {}) as Partial<MeasurementCoverage>;
  const stableId = typeof item.stableId === 'string' ? item.stableId : '';
  return {
    id: stableId || name,
    name: name || stableId || td('common.unknown', locale),
    availability: AVAILABILITY[coverage.availability as number] ?? 'measurement.availability.unknown',
    source: typeof coverage.source === 'string' ? coverage.source : '',
    observedUnixMs: read(coverage as Loose, 'hasObservedUnixMs', 'observedUnixMs'),
  };
}

const text = (value: unknown): string => (typeof value === 'string' ? value : '');
/** Why a value is missing, when the producer said so and the key is one the catalog knows. */
function reasonOf(item: Loose, locale: Locale): string | null {
  const key = isRecord(item.coverage) ? item.coverage.reasonKey : '';
  return typeof key === 'string' && key && hasMessageKey(key) ? td(key, locale) : null;
}

const rows = <T>(items: readonly T[] | undefined, locale: Locale, build: (item: Loose) => MeasurementRow): MeasurementRow[] =>
  Array.isArray(items)
    ? items.filter(isRecord).map((item) => {
        const row = build(item as Loose);
        const note = [row.note, reasonOf(item as Loose, locale)].filter(Boolean).join(' · ');
        return { ...row, note: note || null };
      })
    : [];

export function thermalRows(items: readonly ThermalZoneMeasurement[] | undefined, locale: Locale): MeasurementRow[] {
  const degrees = (value: number) => `${formatNumber(value, locale)} °C`;
  return rows(items, locale, (item) => {
    const temperature = read(item, 'hasTemperature', 'temperatureC');
    const critical = read(item, 'hasCritical', 'criticalC');
    const highest = read(item, 'hasHighestObserved', 'highestObservedC');
    const notes = [
      critical === null ? '' : td('measurement.thermal.critical', locale, { value: degrees(critical) }),
      highest === null ? '' : td('measurement.thermal.highest', locale, { value: degrees(highest) }),
    ].filter(Boolean);
    return { ...frame(item, locale, text(item.displayName)), value: temperature === null ? null : degrees(temperature), note: notes.join(' · ') || null };
  });
}

export function batteryRows(items: readonly BatteryMeasurement[] | undefined, locale: Locale): MeasurementRow[] {
  const mwh = (value: number) => td('measurement.unit.mwh', locale, { value: formatNumber(value, locale) });
  return rows(items, locale, (item) => {
    const relative = item.hasDesignCapacityRelative === true || item.hasFullChargeCapacityRelative === true;
    const conflict = relative && (item.hasDesignCapacity === true || item.hasFullChargeCapacity === true);
    const capacity = (has: string, field: string): number | null => {
      const value = read(item, has, field);
      return value !== null && Number.isInteger(value) && value > 0 && value <= 4294967295 ? value : null;
    };
    const full = conflict ? null : relative ? capacity('hasFullChargeCapacityRelative', 'fullChargeCapacityRelative') : read(item, 'hasFullChargeCapacity', 'fullChargeCapacityMwh');
    const design = conflict ? null : relative ? capacity('hasDesignCapacityRelative', 'designCapacityRelative') : read(item, 'hasDesignCapacity', 'designCapacityMwh');
    const unit = relative ? (value: number) => td('measurement.unit.relative', locale, { value: formatNumber(value, locale) }) : mwh;
    const cycles = read(item, 'hasCycleCount', 'cycleCount');
    const notes = [
      design === null ? '' : td('measurement.battery.design', locale, { value: unit(design) }),
      cycles === null ? '' : td('measurement.battery.cycles', locale, { count: formatNumber(cycles, locale) }),
    ].filter(Boolean);
    return { ...frame(item, locale, text(item.displayName)), value: full === null ? null : unit(full), note: notes.join(' · ') || null };
  });
}

export function bootRows(items: readonly BootMeasurement[] | undefined, locale: Locale): MeasurementRow[] {
  return rows(items, locale, (item) => {
    const duration = read(item, 'hasDuration', 'durationMs');
    const recorded = finite(item.recordedUnixMs) ? formatDateTime(item.recordedUnixMs, locale) : '';
    const value = duration === null ? null : td('measurement.unit.seconds', locale, { value: formatNumber(Math.round(duration / 100) / 10, locale) });
    return { ...frame(item, locale, recorded), value, note: null };
  });
}

const OPER_STATUS: Record<number, MessageKey> = {
  1: 'measurement.network.status.up',
  2: 'measurement.network.status.down',
  3: 'measurement.network.status.testing',
  4: 'measurement.network.status.unknown',
  5: 'measurement.network.status.dormant',
  6: 'measurement.network.status.notPresent',
  7: 'measurement.network.status.lowerLayerDown',
};

const COUNTERS = ['inOctets', 'outOctets', 'inErrors', 'outErrors', 'inDiscards', 'outDiscards'] as const;
function unsigned(value: unknown): bigint | null {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]{0,19})$/.test(value)) return null;
  const number = BigInt(value);
  return number <= 18446744073709551615n ? number : null;
}
function counterValues(value: unknown, locale: Locale): Record<string, string> | null {
  if (!isRecord(value)) return null;
  const result: Record<string, string> = {};
  for (const field of COUNTERS) {
    const number = unsigned(value[field]);
    if (number === null) return null;
    result[field] = new Intl.NumberFormat(locale === 'ar' ? 'ar-IQ-u-nu-latn' : 'en-US').format(number);
  }
  return result;
}
function networkMetrics(item: Loose, locale: Locale): string[] {
  const notes: string[] = [];
  if (typeof item.counterAvailability === 'number') {
    const values = item.counterAvailability === 1 ? counterValues(item.counters, locale) : null;
    notes.push(values ? td('measurement.network.cumulative', locale, values) : td('measurement.network.countersUnavailable', locale));
    const delta = item.counterAvailability === 1 && isRecord(item.counterDelta) ? item.counterDelta : null;
    const counts = delta ? counterValues(delta.counts, locale) : null;
    const elapsed = unsigned(delta?.elapsedMs);
    notes.push(counts && elapsed !== null && elapsed > 0n ? td('measurement.network.delta', locale, {
      ...counts, window: new Intl.NumberFormat(locale === 'ar' ? 'ar-IQ-u-nu-latn' : 'en-US').format(elapsed),
    }) : td('measurement.network.twoSamples', locale));
  }
  if (typeof item.routeAvailability === 'number') {
    if (item.routeAvailability !== 1 || item.hasDefaultRouteV4 !== true || item.hasDefaultRouteV6 !== true)
      notes.push(td('measurement.network.routesUnavailable', locale));
    else {
      if (item.defaultRouteV4 === true) notes.push(td('measurement.network.v4Default', locale));
      if (item.defaultRouteV6 === true) notes.push(td('measurement.network.v6Default', locale));
      if (item.defaultRouteV4 !== true && item.defaultRouteV6 !== true) notes.push(td('measurement.network.noDefault', locale));
    }
  }
  if (item.hasAdminEnabled === true && item.adminEnabled === false) notes.push(td('measurement.network.adminDisabled', locale));
  if (item.hasIpv4Apipa === true && item.ipv4Apipa === true) notes.push(td('measurement.network.apipa', locale));
  return notes;
}

/**
 * One line per adapter: link speed as reported, and its state in words. A down link, an unplugged
 * cable and a virtual adapter are states of an adapter; none is read as "no internet". An
 * unreported media state is not "disconnected".
 */
export function networkRows(items: readonly NetworkAdapterMeasurement[] | undefined, locale: Locale): MeasurementRow[] {
  return rows(items, locale, (item) => {
    const bps = read(item, 'hasLinkSpeed', 'linkSpeedBps');
    const value = bps === null ? null : td('measurement.unit.mbps', locale, { value: formatNumber(bps / 1_000_000, locale) });
    const status = read(item, 'hasOperationalStatus', 'operationalStatus');
    const notes = [
      status === null ? '' : td(OPER_STATUS[status] ?? 'measurement.network.status.unknown', locale),
      item.hasConnected === true ? td(item.connected === true ? 'measurement.network.mediaConnected' : 'measurement.network.mediaDisconnected', locale) : '',
      item.hasIsVirtual === true && item.isVirtual === true ? td('measurement.network.virtual', locale) : '',
      ...networkMetrics(item, locale),
    ].filter(Boolean);
    return { ...frame(item, locale, text(item.displayName)), value, note: notes.join(' · ') || null };
  });
}
