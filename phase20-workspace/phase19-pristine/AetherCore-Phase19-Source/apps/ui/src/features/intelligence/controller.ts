import type { DeepScanHistoryEntry, DeepScanSnapshot, PcRemediationPlan } from '../../lib/contracts';
import { t } from '../../lib/i18n';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState } from '../../platform/stream-state';
import { announce, currentShellState, runBusy } from '../../app/shell-state';

export async function startDeepScan(): Promise<void> {
  await runBusy(async () => {
    const snapshot = await serviceInvoke<DeepScanSnapshot>('start_deep_scan');
    patchStreamState({ deepScan: snapshot });
    announce(t('deepScan.announce.started', currentShellState().locale));
  });
}

export async function cancelDeepScan(scanId: string): Promise<void> {
  if (!scanId) return;
  await runBusy(async () => {
    const snapshot = await serviceInvoke<DeepScanSnapshot>('cancel_deep_scan', { scanId });
    patchStreamState({ deepScan: snapshot });
    announce(t('deepScan.announce.cancelRequested', currentShellState().locale));
  });
}

export async function refreshDeepScanHistory(limit = 12): Promise<void> {
  const history = await serviceInvoke<DeepScanHistoryEntry[]>('get_deep_scan_history', { limit });
  patchStreamState({ deepScanHistory: history });
}

/**
 * Phase 17 only seals an immutable proposal. It intentionally does not execute the actions.
 * Later phases must translate approved candidates through existing domain consent + mutation paths.
 */
export async function sealReviewedRemediationPlan(scanId: string, selectedActionIds: string[]): Promise<PcRemediationPlan | undefined> {
  return runBusy(async () => serviceInvoke<PcRemediationPlan>('seal_remediation_plan', { scanId, selectedActionIds }));
}
