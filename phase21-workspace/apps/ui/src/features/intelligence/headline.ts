import type { DiagnosticsSnapshot } from '../../lib/contracts';
import type { MessageKey } from '../../lib/i18n';

/**
 * The deep scan's headline verdict. P75 trial: a Partial scan (state 4: collectors unavailable)
 * with no findings was headlined "Healthy", a verdict on what was never checked, and an
 * unknown status fell through to "Healthy" as well. A Failed (6) or Cancelled (5) scan has no
 * findings when nothing was collected, so its status reads Healthy; it headlines its state. So
 * does a scan still running (2), which has no findings yet (P76, DBT-P75-091).
 */
export function deepScanHeadlineKey(status: number, state: number): MessageKey {
  if (status === 4) return 'deepScan.status.critical';
  if (status === 3) return 'deepScan.status.action';
  if (status === 2) return 'deepScan.status.attention';
  if (status === 1) {
    if (state === 2) return 'deepScan.state.scanning';
    if (state === 4) return 'deepScan.status.partialClear';
    if (state === 5) return 'deepScan.state.cancelled';
    if (state === 6) return 'deepScan.state.failed';
    return 'deepScan.status.noneFound';
  }
  return 'common.unknown';
}

export type HardwareVerdict = {
  kind: 'notCollected' | 'collecting' | 'unavailable' | 'action' | 'attention' | 'noneFound';
  /** What the measurements covered: every source answered, some did not, or none did. */
  coverage: 'complete' | 'partial' | 'none';
  measured: MessageKey[];
  notMeasured: MessageKey[];
  /** A provider was refused access. That is a coverage gap, not a sign of damage. */
  denied: boolean;
};

const PERMISSION_DENIED = 4;

/**
 * The Hardware page's verdict. It says what was measured, not that the PC is fine: a scan that
 * measured nothing, or only part, is never "healthy" (H3), and "no issues" is always "in the
 * measured checks". Action and denial are separate answers.
 */
export function hardwareVerdict(d: Pick<DiagnosticsSnapshot, 'state' | 'storage' | 'memory' | 'providerFaults'>): HardwareVerdict {
  const measured: MessageKey[] = [];
  const notMeasured: MessageKey[] = [];
  (d.storage.length ? measured : notMeasured).push('hardware.storageDevices');
  (d.memory ? measured : notMeasured).push('hardware.memoryLoad');
  const denied = d.providerFaults.some((fault) => fault.kindCode === PERMISSION_DENIED);
  const base = { measured, notMeasured, denied };
  if (d.state === 'Idle') return { ...base, kind: 'notCollected', coverage: 'none' };
  if (d.state === 'Collecting') return { ...base, kind: 'collecting', coverage: 'none' };
  if (!measured.length) return { ...base, kind: 'unavailable', coverage: 'none' };
  const coverage = notMeasured.length || d.providerFaults.length ? 'partial' : 'complete';
  if (d.storage.some((disk) => disk.severity === 'ActionRequired')) return { ...base, kind: 'action', coverage };
  if (d.storage.some((disk) => disk.severity === 'Attention')) return { ...base, kind: 'attention', coverage };
  return { ...base, kind: 'noneFound', coverage };
}
