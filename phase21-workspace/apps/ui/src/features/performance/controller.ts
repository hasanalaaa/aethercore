import { writable } from 'svelte/store';
import type { BottleneckReport, PerfSnapshot } from '../../lib/contracts';
import { runBusy, setPage } from '../../app/shell-state';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState, streamState } from '../../platform/stream-state';

/** The service's message key for why the last analysis gave nothing; empty when it did not fail. */
export const perfUi = writable({ analysisError: '' });

/** Starts passive sampling and immediately pulls a first snapshot. */
export async function startPerfSampling(): Promise<void> {
  await runBusy(async () => {
    setPage('performance');
    perfUi.set({ analysisError: '' });
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
      perfUi.set({ analysisError: '' });
      lastReport = report;
    } catch (error) {
      // Too few samples is a normal early-window state, but it is said, not shown as an empty report.
      patchStreamState({ bottleneckReport: null });
      perfUi.set({ analysisError: typeof error === 'string' ? error : 'perf.error.analysisFailed' });
      lastReport = null;
    }
  });
  return lastReport;
}
