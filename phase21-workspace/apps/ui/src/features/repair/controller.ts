import { get, writable } from 'svelte/store';
import type { Plan, RepairAssessment, SystemRepairStatus } from '../../lib/contracts';
import { currentShellState, runBusy, setPage } from '../../app/shell-state';
import { refreshServiceSnapshot } from '../../platform/snapshot';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState, streamState } from '../../platform/stream-state';
import { settleAssessment } from './settle';

export const repairStopRequested = writable('');

export const repairUi = writable({ includeDiskScan: true, planDiskScan: null as boolean | null, reviewOpen: false });

export function setIncludeDiskScan(includeDiskScan: boolean): void { repairUi.update((state) => ({ ...state, includeDiskScan })); }
export function closeRepairReview(): void { repairUi.update((state) => ({ ...state, reviewOpen: false })); }
export function openRepairReview(): void { repairUi.update((state) => ({ ...state, reviewOpen: true })); }

function applyAssessmentReply(answer: RepairAssessment, issuedId: string, issuedGeneration: number): void {
  streamState.update((state) => {
    if (state.resetGeneration !== issuedGeneration
      || (state.repairAssessment.assessmentId !== issuedId && state.repairAssessment.assessmentId !== answer.assessmentId)) return state;
    const assessment = settleAssessment(state.repairAssessment, answer);
    return { ...state, repairAssessment: assessment,
      repairAssessmentObservedUnixMs: assessment.assessmentId === state.repairAssessment.assessmentId ? state.repairAssessmentObservedUnixMs : 0 };
  });
}

export async function startRepairAssessment(): Promise<void> {
  setPage('repair');
  await runBusy(async () => {
    patchStreamState({ repairPlan: null, repairStatus: null });
    const issued = get(streamState);
    const answer = await serviceInvoke<RepairAssessment>('start_repair_assessment');
    applyAssessmentReply(answer, issued.repairAssessment.assessmentId, issued.resetGeneration);
  });
}

/** P76 (DBT-P76-007): stops a running assessment; the checks already finished are kept. It names the
 *  assessment on screen (P78-03), so a click that arrives late cannot stop a newer one. */
export async function cancelRepairAssessment(): Promise<void> {
  try {
    const issued = get(streamState);
    const { assessmentId } = issued.repairAssessment;
    const answer = await serviceInvoke<RepairAssessment>('cancel_repair_assessment', { assessmentId });
    if (answer.assessmentId === assessmentId) applyAssessmentReply(answer, assessmentId, issued.resetGeneration);
  } catch {
    /* the stream still carries the running assessment; the next event settles it */
  }
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

/** The acknowledgement says only that a stop was requested; events settle the actual outcome. */
export async function cancelSystemRepair(): Promise<void> {
  const status = get(streamState).repairStatus;
  if (!status || !repairActive()) return;
  const planId = status.planId;
  repairStopRequested.set(planId);
  await runBusy(async () => {
    try {
      await serviceInvoke<SystemRepairStatus | null>('cancel_system_repair', { planId });
    } catch (error) {
      repairStopRequested.set('');
      throw error;
    }
  });
}
