import { get, writable } from 'svelte/store';
import type { CareDomainEligibility, CarePreview, CareRunStatus } from '../../lib/contracts';
import { runBusy, setPage } from '../../app/shell-state';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState, streamState } from '../../platform/stream-state';
import { afterApproval, afterPrepare, isPlanChanged, type CareLoad } from './approval';

/** UI-only state for the prepare -> review -> approve flow. */
export const careUi = writable({
  consentDialogOpen: false,
  /** The local scan care asked for has not finished; the page continues when it does. */
  preparing: false,
  /** Why each domain has, or has not, something prepared (P79-04A). */
  domains: [] as CareDomainEligibility[],
});

/** Whether the last read of the plan worked; the panel says "unavailable" for a failed one. */
export const careLoad = writable<CareLoad>('idle');

/** Pulls the current care status (also the deterministic plan preview). */
export async function loadCareStatus(): Promise<void> {
  careLoad.set('loading');
  await runBusy(async () => {
    try {
      patchStreamState({ careStatus: await serviceInvoke<CareRunStatus>('get_care_status') });
      careLoad.set('loaded');
    } catch (error) {
      careLoad.set('failed');
      throw error;
    }
  });
}

let preparing = false;

/**
 * P79-04B: asks the service to prepare what care could run (P79-04A: it reads and prepares only).
 * A prepared plan with automatic work opens the approval dialog, on the plan as the service
 * composes it now, so the owner approves the plan they are shown; while the local scan runs the
 * page says so and continues when the scan ends; otherwise it says why nothing is prepared.
 * Nothing runs until the owner confirms.
 */
export async function prepareCare(): Promise<void> {
  if (preparing) return;
  preparing = true;
  try {
    const preview = await runBusy(() => serviceInvoke<CarePreview>('prepare_care_preview'));
    if (!preview) return;
    patchStreamState({ careStatus: preview.status });
    const next = afterPrepare(preview);
    careUi.update((state) => ({ ...state, domains: preview.domains, preparing: next === 'wait', consentDialogOpen: next === 'review' }));
    if (next !== 'review') setPage('activity');
  } finally {
    preparing = false;
  }
}

export function closeCareConsent(): void {
  careUi.update((state) => ({ ...state, consentDialogOpen: false }));
}

/**
 * Approves the plan shown for one run, then starts that run in the same user gesture. The
 * grant names the plan shown and the service refuses it if the plan has changed since; then
 * nothing starts and the new plan is shown for approval. A refused or failed call is reported by runBusy.
 */
export async function authorizeAndStartCare(): Promise<void> {
  const shown = get(streamState).careStatus;
  closeCareConsent();
  await runBusy(async () => {
    let granted: CareRunStatus;
    try {
      // The grant names the plan shown; the service approves nothing if it has changed.
      granted = await serviceInvoke<CareRunStatus>('grant_care_session_consent', { planDigestSha256: shown?.planDigestSha256 ?? '' });
    } catch (error) {
      if (!isPlanChanged(error)) throw error;
      const current = await serviceInvoke<CareRunStatus>('get_care_status');
      patchStreamState({ careStatus: { ...current, state: 'AwaitingConsent', summaryKey: 'care.summary.planChanged' } });
      setPage('activity');
      return;
    }
    const next = afterApproval(shown, granted);
    if (!next.start) {
      patchStreamState({ careStatus: next.status });
      setPage('activity');
      return;
    }
    const status = await serviceInvoke<CareRunStatus>('start_care_run');
    patchStreamState({ careStatus: status });
    // The report lives in the care panel on the Activity page (§51.3), not where the click was.
    setPage('activity');
  });
}

/** Cancels at the next step boundary; remaining steps are cited as skipped. */
export async function cancelCare(): Promise<void> {
  await runBusy(async () => {
    try {
      const status = await serviceInvoke<CareRunStatus>('cancel_care_run');
      patchStreamState({ careStatus: status });
    } catch {
      /* offline: nothing to cancel */
    }
  });
}
