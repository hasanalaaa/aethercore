import type { RepairAssessment } from '../../lib/contracts';

export function assessmentElapsedMs(assessment: RepairAssessment, observedUnixMs: number,
  sinceObservationMs = 0, nowUnixMs = Date.now()): number | null {
  const start = assessment.startedUnixMs;
  const end = assessment.state === 'Scanning' ? observedUnixMs : assessment.completedUnixMs;
  if (!Number.isSafeInteger(start) || start <= 0 || !Number.isSafeInteger(end)
    || end < start || end > nowUnixMs || !Number.isFinite(sinceObservationMs) || sinceObservationMs < 0)
    return null;
  return end - start + (assessment.state === 'Scanning' ? sinceObservationMs : 0);
}

/** Local display animation only: no IPC, renderer polling, or inferred backend progress.
 *  Monotonic time includes background pauses; frame count is never a clock. */
export function runElapsedClock(update: (elapsedMs: number) => void, initialMs = 0): () => void {
  const anchor = performance.now();
  let frame = 0, active = true, lastSecond = Math.floor(initialMs / 1000);
  function paint(): void {
    if (!active) return;
    const elapsed = initialMs + Math.max(0, performance.now() - anchor);
    const second = Math.floor(elapsed / 1000);
    if (second !== lastSecond) { lastSecond = second; update(second * 1000); }
    if (active) frame = requestAnimationFrame(paint);
  }
  frame = requestAnimationFrame(paint);
  return () => { active = false; cancelAnimationFrame(frame); };
}
