import type { BottleneckEvidenceRef } from '../../lib/contracts';
import { formatNumber, hasMessageKey, td, type Locale } from '../../lib/i18n';

const MIB = 1024 * 1024;

/**
 * One piece of evidence as a sentence with units. The rules report values in the unit of their
 * counter (basis points, microseconds, bytes); the page used to print them raw next to the
 * counter's internal name. A fact nobody labelled falls back to the bare pair, never to an error.
 */
export function formatEvidence(ev: BottleneckEvidenceRef, locale: Locale): string {
  const key = `perf.fact.${ev.factKey}`;
  if (!hasMessageKey(key)) return `${formatNumber(ev.observedValue, locale)} / ${formatNumber(ev.threshold, locale)}`;
  return td(key, locale, { observed: withUnit(ev.factKey, ev.observedValue, locale), threshold: withUnit(ev.factKey, ev.threshold, locale) });
}

function withUnit(factKey: string, value: number, locale: Locale): string {
  if (factKey.includes('Bp.')) return `${formatNumber(Math.round(value / 100), locale)}%`;
  if (factKey.endsWith('Us')) return `${formatNumber(Math.round(value / 1000), locale)} ${td('perf.unit.ms', locale)}`;
  if (factKey.endsWith('Bytes')) return `${formatNumber(Math.round(value / MIB), locale)} ${td('perf.unit.mb', locale)}`;
  return formatNumber(Math.round(value), locale);
}
