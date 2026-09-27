import type { MessageKey } from '../../lib/i18n';

/**
 * The deep scan's headline verdict. P75 trial: a Partial scan (state 4: collectors unavailable)
 * with no findings was headlined "Healthy", a verdict on what was never checked, and an
 * unknown status fell through to "Healthy" as well.
 */
export function deepScanHeadlineKey(status: number, state: number): MessageKey {
  if (status === 4) return 'deepScan.status.critical';
  if (status === 3) return 'deepScan.status.action';
  if (status === 2) return 'deepScan.status.attention';
  if (status === 1) return state === 4 ? 'deepScan.status.partialClear' : 'deepScan.status.healthy';
  return 'common.unknown';
}
