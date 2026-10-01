import type { StorageTelemetry } from '../../lib/contracts';
import type { MessageKey } from '../../lib/i18n';

/**
 * What THIS disk did not report, judged by its own bus. A SATA disk is not charged with counters only
 * NVMe has, and one disk's gap is never folded into a statement about the machine.
 */
export function missingDiskMetrics(disk: Pick<StorageTelemetry, 'busType' | 'reliability'>): MessageKey[] {
  const r = disk.reliability;
  const nvme = disk.busType.toLowerCase() === 'nvme';
  const checks: [boolean, MessageKey][] = [
    [!r?.hasTemperature, 'hardware.temperature'],
    [!r?.hasWear, 'hardware.wear'],
    [!r?.hasPowerOnHours, 'hardware.powerOn'],
    [!r?.hasReadErrorsUncorrected, 'hardware.uncorrectedReads'],
    [!r?.hasWriteErrorsUncorrected, 'hardware.uncorrectedWrites'],
    [nvme && !r?.hasNvmeAvailableSpare, 'hardware.nvmeSpare'],
    [nvme && !r?.hasNvmeCriticalWarning, 'hardware.nvmeFlags'],
  ];
  return checks.filter(([missing]) => missing).map(([, key]) => key);
}

/**
 * The first line of advice for a disk. Evidence of data risk asks for a backup before anything else
 * (no optimizer writes to a suspect disk); a disk with no verdict says it is unknown, not fine.
 */
export function diskAdvice(disk: Pick<StorageTelemetry, 'severity'>): MessageKey | null {
  if (disk.severity === 'ActionRequired') return 'hardware.disk.backupFirst';
  if (disk.severity === 'Unknown') return 'hardware.disk.unsupported';
  return null;
}
