import { writable } from 'svelte/store';
import type { BottleneckReport, OptimizationPlanSnapshot, PerfSnapshot } from '../../lib/contracts';
import { runBusy, setPage } from '../../app/shell-state';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState, streamState } from '../../platform/stream-state';

export const perfUi = writable({
  selectedFindingIds: [] as string[],
  plan: null as OptimizationPlanSnapshot | null,
});

export function setSelectedFindingIds(ids: string[]): void {
  perfUi.update((state) => ({ ...state, selectedFindingIds: ids }));
}

/** Starts passive sampling and immediately pulls a first snapshot. */
export async function startPerfSampling(): Promise<void> {
  await runBusy(async () => {
    setPage('performance');
    const snapshot = await serviceInvoke<PerfSnapshot>('get_performance_snapshot');
    patchStreamState({ performance: snapshot });
    await serviceInvoke<void>('start_perf_sampling', { intervalMs: 1000 });
    patchStreamState({ perfSampling: true });
  });
}

export async function stopPerfSampling(): Promise<void> {
  await runBusy(async () => {
    await serviceInvoke<void>('stop_perf_sampling');
    patchStreamState({ perfSampling: false });
  });
}

/** Runs bottleneck analysis over the accumulated ring window. */
let lastReport: BottleneckReport | null = null;

export async function analyzeBottlenecks(): Promise<BottleneckReport | null> {
  await runBusy(async () => {
    setPage('performance');
    try {
      const report = await serviceInvoke<BottleneckReport>('get_bottleneck_report');
      patchStreamState({ bottleneckReport: report });
      lastReport = report;
    } catch {
      // Insufficient evidence is a normal early-window state; surface as empty report.
      patchStreamState({ bottleneckReport: null });
      lastReport = null;
    }
  });
  return lastReport;
}

let lastPlan: OptimizationPlanSnapshot | null = null;

export async function reviewOptimizationPlan(findingIds: string[]): Promise<OptimizationPlanSnapshot | null> {
  await runBusy(async () => {
    try {
      const plan = await serviceInvoke<OptimizationPlanSnapshot>('create_optimization_plan', {
        selectedFindingIds: findingIds,
      });
      perfUi.update((state) => ({ ...state, plan }));
      lastPlan = plan;
    } catch {
      lastPlan = null;
    }
  });
  return lastPlan;
}

export function closeOptimizationReview(): void {
  perfUi.update((state) => ({ ...state, plan: null, selectedFindingIds: [] }));
}
