import { get, writable } from 'svelte/store';
import type { Plan, RepairAssessment, SystemRepairStatus } from '../../lib/contracts';
import { currentShellState, runBusy, setPage } from '../../app/shell-state';
import { refreshServiceSnapshot } from '../../platform/snapshot';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState, streamState } from '../../platform/stream-state';

export const repairUi = writable({ includeDiskScan: true, planDiskScan: null as boolean | null, reviewOpen: false });

export function setIncludeDiskScan(includeDiskScan: boolean): void { repairUi.update((state) => ({ ...state, includeDiskScan })); }
export function closeRepairReview(): void { repairUi.update((state) => ({ ...state, reviewOpen: false })); }
export function openRepairReview(): void { repairUi.update((state) => ({ ...state, reviewOpen: true })); }

export async function startRepairAssessment(): Promise<void> {
  setPage('repair');
  await runBusy(async () => {
    patchStreamState({ repairPlan: null, repairStatus: null });
    const repairAssessment = await serviceInvoke<RepairAssessment>('start_repair_assessment');
    patchStreamState({ repairAssessment });
  });
}

export async function reviewSystemRepair(): Promise<void> {
  const { repairAssessment } = get(streamState);
  if (repairAssessment.state !== 'Ready' || !repairAssessment.assessmentId) return;
  const runDiskScan = get(repairUi).includeDiskScan;
  await runBusy(async () => {
    const repairPlan = await serviceInvoke<Plan>('create_system_repair_plan', { assessmentId: repairAssessment.assessmentId, runDiskScan });
    patchStreamState({ repairPlan });
    repairUi.set({ includeDiskScan: runDiskScan, planDiskScan: runDiskScan, reviewOpen: true });
    await refreshServiceSnapshot();
  });
}

export async function authorizeAndRepair(): Promise<void> {
  const { repairPlan } = get(streamState);
  if (!repairPlan) return;
  await runBusy(async () => {
    await serviceInvoke<void>('approve_plan_with_uac', { planId: repairPlan.id, locale: currentShellState().locale });
    await refreshServiceSnapshot();
    const repairStatus = await serviceInvoke<SystemRepairStatus>('start_system_repair', { planId: repairPlan.id });
    patchStreamState({ repairStatus });
    closeRepairReview();
    await refreshServiceSnapshot();
  });
}

export function repairActive(): boolean {
  const status = get(streamState).repairStatus;
  return !!status && !['Completed', 'Failed'].includes(status.planState);
}
