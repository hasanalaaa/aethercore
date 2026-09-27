import type { CareRunStatus } from '../../lib/contracts';

/**
 * DBT-P75-045: the service approves the care plan it composes when the owner clicks, and that
 * approval runs the plan once. Start only if it is the plan the owner was shown; otherwise
 * show the new plan and ask again.
 */
export function afterApproval(shown: CareRunStatus | null, granted: CareRunStatus): { start: boolean; status: CareRunStatus } {
  if (shown && shown.planDigestSha256 !== '' && granted.planDigestSha256 === shown.planDigestSha256) {
    return { start: true, status: granted };
  }
  return {
    start: false,
    status: { ...granted, state: 'AwaitingConsent', sessionConsentGranted: false, summaryKey: 'care.summary.planChanged' },
  };
}

/** The steps one approval runs: Auto (level 0) only. */
export function approvedStepCount(status: CareRunStatus | null): number {
  return status ? status.steps.filter((step) => step.safetyLevel <= 0).length : 0;
}

/**
 * The service refuses a grant whose plan changed since the owner saw it (DBT-P75-045, P75
 * review #39): the desktop surfaces the refusal as its message key. That is not a failure to
 * report; it is a new plan to show.
 */
export function isPlanChanged(error: unknown): boolean {
  return String(error).includes('care.error.planChanged');
}
