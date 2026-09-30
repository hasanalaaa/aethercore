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
