import type { DiagnosticsSnapshot } from '../../lib/contracts';

export type MemoryTestSummary = {
  /**
   * `noErrors` and `errors` are the latest Windows Memory Diagnostic result in the event window;
   * `notTested` means the log was read and holds none; `unknown` means the log could not be read.
   */
  state: 'noErrors' | 'errors' | 'notTested' | 'unknown';
  unixMs: number | null;
  windowDays: number;
};

const PROVIDER = 'Microsoft-Windows-MemoryDiagnostics-Results';

/**
 * What Windows Memory Diagnostic last reported. A result is a dated statement about that run, not a
 * statement about the memory now, and the absence of a result is "not tested", never a pass. The
 * snapshot's `eventWindowDays` is 0 exactly when the event log could not be read.
 */
export function memoryTestSummary(d: Pick<DiagnosticsSnapshot, 'events' | 'eventWindowDays'>): MemoryTestSummary {
  const windowDays = d.eventWindowDays;
  if (!windowDays) return { state: 'unknown', unixMs: null, windowDays };
  const results = (d.events ?? []).filter((e) => e.category === 'MemoryTestResult' && e.provider === PROVIDER && (e.eventId === 1201 || e.eventId === 1202));
  const latest = results.reduce<(typeof results)[number] | null>((best, e) => (!best || e.recordedUnixMs >= best.recordedUnixMs ? e : best), null);
  if (!latest) return { state: 'notTested', unixMs: null, windowDays };
  return { state: latest.eventId === 1202 ? 'errors' : 'noErrors', unixMs: latest.recordedUnixMs, windowDays };
}
