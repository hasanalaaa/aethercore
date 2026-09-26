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
