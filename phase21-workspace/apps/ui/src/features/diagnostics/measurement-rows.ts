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
    const full = read(item, 'hasFullChargeCapacity', 'fullChargeCapacityMwh');
    const design = read(item, 'hasDesignCapacity', 'designCapacityMwh');
    const cycles = read(item, 'hasCycleCount', 'cycleCount');
    const notes = [
      design === null ? '' : td('measurement.battery.design', locale, { value: mwh(design) }),
      cycles === null ? '' : td('measurement.battery.cycles', locale, { count: formatNumber(cycles, locale) }),
    ].filter(Boolean);
    return { ...frame(item, locale, text(item.displayName)), value: full === null ? null : mwh(full), note: notes.join(' · ') || null };
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

export function networkRows(items: readonly NetworkAdapterMeasurement[] | undefined, locale: Locale): MeasurementRow[] {
  return rows(items, locale, (item) => {
    const bps = read(item, 'hasLinkSpeed', 'linkSpeedBps');
    const value = bps === null ? null : td('measurement.unit.mbps', locale, { value: formatNumber(bps / 1_000_000, locale) });
    return { ...frame(item, locale, text(item.displayName)), value, note: null };
  });
}
