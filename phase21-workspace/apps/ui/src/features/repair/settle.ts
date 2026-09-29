import type { RepairAssessment } from '../../lib/contracts';

/**
 * An answer to `start_repair_assessment` or `cancel_repair_assessment` can arrive after the event
 * stream has already said the assessment ended. The stream is ordered and the answer is not, so an
 * answer that only repeats "Scanning" for an assessment the screen holds as finished is stale.
 */
export function settleAssessment(current: RepairAssessment, answer: RepairAssessment): RepairAssessment {
  const finished = current.state !== 'Scanning' && current.state !== 'Idle';
  return finished && answer.state === 'Scanning' && answer.assessmentId === current.assessmentId ? current : answer;
}

/** m:ss, or h:mm:ss; empty when the start is unknown, never negative. */
export function elapsedLabel(startedUnixMs: number, nowUnixMs: number): string {
  if (!(startedUnixMs > 0)) return '';
  const total = Math.max(0, Math.floor((nowUnixMs - startedUnixMs) / 1000));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = String(total % 60).padStart(2, '0');
  return hours ? `${hours}:${String(minutes).padStart(2, '0')}:${seconds}` : `${minutes}:${seconds}`;
}
