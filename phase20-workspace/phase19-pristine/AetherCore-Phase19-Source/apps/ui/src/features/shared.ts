import type { DriverCandidate, DriverDevice } from '../lib/contracts';
import { driverStateLabel, driverTargetEvidence, driverTargetLabel, formatDateTime, formatNumber, t, type Locale } from '../lib/i18n';

export function formatBytes(bytes: number, locale: Locale = 'en'): string {
  if (!bytes) return '—';
  const units = ['B', 'KB', 'MB', 'GB'] as const;
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
  const digits = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${formatNumber(value, locale, { minimumFractionDigits: digits, maximumFractionDigits: digits })} ${units[unit]}`;
}

export function formatRange(min: number, max: number, locale: Locale = 'en'): string {
  if (!min && !max) return t('drivers.sizeNotReported', locale);
  if (!min || min === max) return formatBytes(max || min, locale);
  return `${formatBytes(min, locale)} – ${formatBytes(max, locale)}`;
}

export function formatWhen(ms: number, locale: Locale = 'en'): string { return formatDateTime(ms, locale); }
export function shortDigest(digest?: string): string { return digest ? `${digest.slice(0, 10)}…${digest.slice(-8)}` : '—'; }
export function stageTone(stage: string): 'bad' | 'warn' | 'ok' {
  return stage.toLowerCase().includes('failed') || stage === 'RecoveryRequired' ? 'bad' : stage === 'RebootPending' ? 'warn' : 'ok';
}
export function targetLabel(candidate: DriverCandidate, locale: Locale = 'en'): string { return driverTargetLabel(candidate, locale); }
export function targetEvidence(candidate: DriverCandidate, locale: Locale = 'en'): string { return driverTargetEvidence(candidate, locale); }
export function stateLabel(device: DriverDevice, locale: Locale = 'en'): string { return driverStateLabel(device, locale); }
