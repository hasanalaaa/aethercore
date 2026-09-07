import type { PerfSnapshot } from '../../lib/contracts';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState } from '../../platform/stream-state';

/**
 * The Overview's own reading loop — why this screen fills itself by READING
 * rather than by starting the sampler.
 *
 * §51.1 traced the switch the brief asked about. `start_perf_sampling` spawns
 * `aether-perf-sampler` in the service (`PerformanceRing::start`), which pushes
 * into an in-process ring and **publishes nothing**: the only producer of a
 * `performanceSnapshot` kernel event in the whole product is the
 * `GetPerformanceSnapshot` handler (`router.rs`). Starting the sampler from
 * here would therefore have cost a background thread and still left the
 * instrument blank. Reading is what fills it — and the same handler calls
 * `ensure_sample`, which re-samples when the newest reading is older than the
 * requested interval (DBT-P42-008), so a read is never served a stale one.
 *
 * **Cadence is a decision with a measured price**, per §42 and §45. One
 * `sample()` on this machine, release build, ten ticks after a warm-up:
 * min 129.9 ms · mean 230.8 ms · max 624.2 ms, of which ~120 ms is the
 * provider's mandatory two-observation window. At 5 s that is under 5% of one
 * core while this screen is open, and exactly nothing when it is not — the loop
 * lives and dies with the component. The Performance screen keeps its 1 s
 * sampler: that is a screen you sit and watch, and this is a summary.
 *
 * Reading telemetry is not a mutation. This path touches no `MutationSupervisor`,
 * opens no commit fence, writes no journal entry and takes no restore point;
 * `PerfPlatform::sample` reads counters. The consent this product asks for is
 * consent to CHANGE the machine, and nothing here changes it.
 */
export const OVERVIEW_READ_INTERVAL_MS = 5_000;

/**
 * A response that carries no capture time is not a reading, and must not be
 * allowed to overwrite one. Validated here, at the boundary, because everything
 * downstream trusts `capturedUnixMs > 0` to mean "measured".
 */
function isReading(snapshot: PerfSnapshot | undefined): snapshot is PerfSnapshot {
  return Boolean(snapshot) && typeof snapshot?.capturedUnixMs === 'number' && snapshot.capturedUnixMs > 0;
}

/**
 * One read. Deliberately NOT wrapped in `runBusy`: a refresh the user did not
 * ask for must not disable the screen's buttons or clear the error banner from
 * an operation they did.
 */
export async function readTelemetryNow(): Promise<boolean> {
  try {
    const snapshot = await serviceInvoke<PerfSnapshot>('get_performance_snapshot');
    if (!isReading(snapshot)) return false;
    patchStreamState({ performance: snapshot });
    return true;
  } catch (error) {
    // Not swallowed: the console keeps the reason, and the caller stops the
    // loop. On screen the channels stay at em dash, which is the truth.
    console.error('[AetherCore] overview telemetry read failed', error);
    return false;
  }
}

/**
 * Reads immediately, then every [`OVERVIEW_READ_INTERVAL_MS`] until disposed.
 * A failed read stops the loop rather than retrying into a dead service every
 * five seconds for as long as the window is open; the empty state's own control
 * is how the user asks again.
 */
export function startOverviewTelemetry(): () => void {
  let timer: ReturnType<typeof setInterval> | undefined;
  // The first read is in flight when the user can already navigate away, so
  // disposal has to be recorded rather than inferred from `timer`.
  let disposed = false;
  const stop = (): void => {
    disposed = true;
    if (timer !== undefined) clearInterval(timer);
    timer = undefined;
  };
  void readTelemetryNow().then((ok) => {
    if (disposed || !ok) return;
    timer = setInterval(() => { void readTelemetryNow().then((again) => { if (!again) stop(); }); }, OVERVIEW_READ_INTERVAL_MS);
  });
  return stop;
}
