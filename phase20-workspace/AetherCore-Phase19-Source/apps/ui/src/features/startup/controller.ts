import { get, writable } from 'svelte/store';
import type { Plan, StartupDecision, StartupHistoryEntry, StartupItem, StartupSnapshot, StartupStatus } from '../../lib/contracts';
import { runBusy, setError, setPage, shellState } from '../../app/shell-state';
import { refreshServiceSnapshot } from '../../platform/snapshot';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState, streamState } from '../../platform/stream-state';
import { t } from '../../lib/i18n';

export const startupUi = writable({
  decisions: {} as Record<string, StartupDecision>,
  decisionScan: '',
  reviewOpen: false,
  serviceConfirmed: false,
  restoreEntry: null as StartupHistoryEntry | null,
  restoreOpen: false,
  restorePlan: null as Plan | null,
  restoreServiceConfirmed: false,
});

function syncDefaults(snapshot: StartupSnapshot): void {
  const ui = get(startupUi);
  if (!snapshot.scanId || snapshot.state !== 'Ready' || snapshot.scanId === ui.decisionScan) return;
  const decisions: Record<string, StartupDecision> = {};
  for (const item of snapshot.items) decisions[item.itemId] = 'Unreviewed';
  startupUi.update((state) => ({ ...state, decisions, decisionScan: snapshot.scanId, serviceConfirmed: false }));
}
streamState.subscribe((state) => syncDefaults(state.startupSnapshot));

export function setStartupDecision(item: StartupItem, decision: StartupDecision): void {
  if (decision === 'Disable' && (!item.manageable || item.protected)) return;
  startupUi.update((state) => {
    const decisions = { ...state.decisions, [item.itemId]: decision };
    const serviceStillSelected = get(streamState).startupSnapshot.items.some((candidate) => decisions[candidate.itemId] === 'Disable' && candidate.serviceChange);
    return { ...state, decisions, serviceConfirmed: serviceStillSelected ? state.serviceConfirmed : false };
  });
}

export function setStartupServiceConfirmed(serviceConfirmed: boolean): void { startupUi.update((state) => ({ ...state, serviceConfirmed })); }
export function setRestoreServiceConfirmed(restoreServiceConfirmed: boolean): void { startupUi.update((state) => ({ ...state, restoreServiceConfirmed })); }
export function selectedStartupDisables(): StartupItem[] {
  const { startupSnapshot } = get(streamState);
  const { decisions } = get(startupUi);
  return startupSnapshot.items.filter((item) => decisions[item.itemId] === 'Disable');
}
export function selectedStartupServices(): StartupItem[] { return selectedStartupDisables().filter((item) => item.serviceChange); }
export function closeStartupReview(): void { startupUi.update((state) => ({ ...state, reviewOpen: false })); }
export function openStartupReview(): void { startupUi.update((state) => ({ ...state, reviewOpen: true })); }
export function closeStartupRestore(): void { startupUi.update((state) => ({ ...state, restoreOpen: false })); }
export function finalizeStartupRestore(): void { startupUi.update((state) => ({ ...state, restoreEntry: null, restoreOpen: false, restorePlan: null, restoreServiceConfirmed: false })); }

export async function startStartupScan(): Promise<void> {
  setPage('startup');
  await runBusy(async () => {
    patchStreamState({ startupPlan: null, startupStatus: null });
    const startupSnapshot = await serviceInvoke<StartupSnapshot>('start_startup_scan');
    patchStreamState({ startupSnapshot });
    startupUi.set({ decisions: {}, decisionScan: '', reviewOpen: false, serviceConfirmed: false, restoreEntry: null, restoreOpen: false, restorePlan: null, restoreServiceConfirmed: false });
  });
}

export async function reviewStartupPlan(): Promise<void> {
  const { startupSnapshot } = get(streamState);
  const selectedItems = selectedStartupDisables();
  const ui = get(startupUi);
  if (startupSnapshot.state !== 'Ready' || !selectedItems.length) return;
  if (selectedItems.some((item) => item.serviceChange) && !ui.serviceConfirmed) {
    setError(t('startup.error.serviceConfirmation', get(shellState).locale));
    return;
  }
  await runBusy(async () => {
    const startupPlan = await serviceInvoke<Plan>('create_startup_plan', {
      scanId: startupSnapshot.scanId,
      inventoryEpoch: startupSnapshot.inventoryEpoch,
      decisions: startupSnapshot.items.map((item) => ({ itemId: item.itemId, decision: ui.decisions[item.itemId] ?? 'Unreviewed' })),
      confirmServiceChanges: ui.serviceConfirmed,
    });
    patchStreamState({ startupPlan });
    startupUi.update((state) => ({ ...state, reviewOpen: true }));
    await refreshServiceSnapshot();
  });
}

export async function authorizeAndApplyStartup(): Promise<void> {
  const { startupPlan } = get(streamState);
  if (!startupPlan) return;
  await runBusy(async () => {
    await serviceInvoke<void>('approve_plan_with_uac', { planId: startupPlan.id, locale: get(shellState).locale });
    await refreshServiceSnapshot();
    const startupStatus = await serviceInvoke<StartupStatus>('start_startup_changes', { planId: startupPlan.id });
    patchStreamState({ startupStatus });
    closeStartupReview();
    await refreshServiceSnapshot();
  });
}

export function openStartupRestore(entry: StartupHistoryEntry): void {
  startupUi.update((state) => ({ ...state, restoreEntry: entry, restoreOpen: true, restorePlan: null, restoreServiceConfirmed: false }));
}

export async function prepareStartupRestore(): Promise<void> {
  const { restoreEntry, restoreServiceConfirmed } = get(startupUi);
  if (!restoreEntry) return;
  if (restoreEntry.kind === 'Service' && !restoreServiceConfirmed) {
    setError(t('startup.error.restoreConfirmation', get(shellState).locale));
    return;
  }
  await runBusy(async () => {
    const restorePlan = await serviceInvoke<Plan>('create_startup_restore_plan', {
      changeId: restoreEntry.changeId,
      confirmServiceChanges: restoreServiceConfirmed,
    });
    startupUi.update((state) => ({ ...state, restorePlan }));
  });
}

export async function authorizeAndRestoreStartup(): Promise<void> {
  const { restorePlan } = get(startupUi);
  if (!restorePlan) return;
  await runBusy(async () => {
    await serviceInvoke<void>('approve_plan_with_uac', { planId: restorePlan.id, locale: get(shellState).locale });
    await refreshServiceSnapshot();
    const startupStatus = await serviceInvoke<StartupStatus>('start_startup_changes', { planId: restorePlan.id });
    patchStreamState({ startupPlan: restorePlan, startupStatus });
    closeStartupRestore();
    await refreshServiceSnapshot();
  });
}

export function startupActive(): boolean {
  const status = get(streamState).startupStatus;
  return !!status && !['Completed', 'Failed'].includes(status.planState);
}
