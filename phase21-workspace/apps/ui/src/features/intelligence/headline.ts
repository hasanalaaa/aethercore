import type { MessageKey } from '../../lib/i18n';

/**
 * The deep scan's headline verdict. P75 trial: a Partial scan (state 4: collectors unavailable)
 * with no findings was headlined "Healthy", a verdict on what was never checked, and an
 * unknown status fell through to "Healthy" as well. A Failed (6) or Cancelled (5) scan has no
 * findings when nothing was collected, so its status reads Healthy; it headlines its state.
 */
export function deepScanHeadlineKey(status: number, state: number): MessageKey {
  if (status === 4) return 'deepScan.status.critical';
  if (status === 3) return 'deepScan.status.action';
  if (status === 2) return 'deepScan.status.attention';
  if (status === 1) {
    if (state === 4) return 'deepScan.status.partialClear';
    if (state === 5) return 'deepScan.state.cancelled';
    if (state === 6) return 'deepScan.state.failed';
    return 'deepScan.status.healthy';
  }
  return 'common.unknown';
}
