import { get, writable } from 'svelte/store';
import type { CleanupCandidate, CleanupSnapshot, CleanupStatus, Plan } from '../../lib/contracts';
import { currentShellState, runBusy, setPage } from '../../app/shell-state';
import { refreshServiceSnapshot } from '../../platform/snapshot';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState, streamState } from '../../platform/stream-state';

export const cleanupUi = writable({
  selected: {} as Record<string, boolean>,
  selectionScan: '',
  reviewOpen: false,
  planExpectedBytes: 0,
  planExplicit: null as boolean | null,
});

function syncDefaults(snapshot: CleanupSnapshot): void {
  const ui = get(cleanupUi);
  if (!snapshot.scanId || snapshot.scanId === ui.selectionScan || snapshot.state !== 'Ready') return;
  const selected: Record<string, boolean> = {};
  for (const candidate of snapshot.candidates) selected[candidate.candidateId] = candidate.selectedByDefault && !candidate.requiresExplicitConfirmation;
  cleanupUi.update((state) => ({ ...state, selected, selectionScan: snapshot.scanId }));
}
streamState.subscribe((state) => syncDefaults(state.cleanupSnapshot));

export function selectedCleanupCandidates(): CleanupCandidate[] {
  const { cleanupSnapshot } = get(streamState);
  const { selected } = get(cleanupUi);
  return cleanupSnapshot.candidates.filter((candidate) => selected[candidate.candidateId]);
}
export function selectedCleanupBytes(): number { return selectedCleanupCandidates().reduce((sum, candidate) => sum + candidate.reclaimableBytes, 0); }
export function cleanupHasExplicitSelection(): boolean { return selectedCleanupCandidates().some((candidate) => candidate.requiresExplicitConfirmation); }
export function toggleCleanup(candidateId: string, checked: boolean): void { cleanupUi.update((state) => ({ ...state, selected: { ...state.selected, [candidateId]: checked } })); }
export function closeCleanupReview(): void { cleanupUi.update((state) => ({ ...state, reviewOpen: false })); }
export function openCleanupReview(): void { cleanupUi.update((state) => ({ ...state, reviewOpen: true })); }

export async function startCleanupScan(): Promise<void> {
  setPage('cleanup');
  await runBusy(async () => {
    patchStreamState({ cleanupPlan: null, cleanupStatus: null });
    const cleanupSnapshot = await serviceInvoke<CleanupSnapshot>('start_cleanup_scan');
    patchStreamState({ cleanupSnapshot });
    cleanupUi.set({ selected: {}, selectionScan: '', reviewOpen: false, planExpectedBytes: 0, planExplicit: null });
  });
}

export async function reviewCleanup(): Promise<void> {
  const { cleanupSnapshot } = get(streamState);
  const chosen = selectedCleanupCandidates();
  if (cleanupSnapshot.state !== 'Ready' || !chosen.length) return;
  await runBusy(async () => {
    const planExpectedBytes = chosen.reduce((sum, candidate) => sum + candidate.reclaimableBytes, 0);
    const planExplicit = chosen.some((candidate) => candidate.requiresExplicitConfirmation);
    const cleanupPlan = await serviceInvoke<Plan>('create_cleanup_plan', {
      scanId: cleanupSnapshot.scanId,
      inventoryEpoch: cleanupSnapshot.inventoryEpoch,
      candidateIds: chosen.map((candidate) => candidate.candidateId),
    });
    patchStreamState({ cleanupPlan });
    cleanupUi.update((state) => ({ ...state, reviewOpen: true, planExpectedBytes, planExplicit }));
    await refreshServiceSnapshot();
  });
}

export async function authorizeAndCleanup(): Promise<void> {
  const { cleanupPlan } = get(streamState);
  if (!cleanupPlan) return;
  await runBusy(async () => {
    await serviceInvoke<void>('approve_plan_with_uac', { planId: cleanupPlan.id, locale: currentShellState().locale });
    await refreshServiceSnapshot();
    const cleanupStatus = await serviceInvoke<CleanupStatus>('start_cleanup', { planId: cleanupPlan.id });
    patchStreamState({ cleanupStatus });
    closeCleanupReview();
    await refreshServiceSnapshot();
  });
}

export function cleanupActive(): boolean {
  const status = get(streamState).cleanupStatus;
  return !!status && !['Completed', 'Failed'].includes(status.planState);
}
