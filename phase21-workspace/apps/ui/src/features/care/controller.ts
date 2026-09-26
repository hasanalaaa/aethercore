import { get, writable } from 'svelte/store';
import type { CareRunStatus } from '../../lib/contracts';
import { runBusy, setPage } from '../../app/shell-state';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState, streamState } from '../../platform/stream-state';
import { afterApproval } from './approval';

/** UI-only dialog state for the session-consent flow. */
export const careUi = writable({
  consentDialogOpen: false,
});

/** Pulls the current care status (also the deterministic plan preview). */
export async function loadCareStatus(): Promise<void> {
  await runBusy(async () => {
    setPage('overview');
    try {
      const status = await serviceInvoke<CareRunStatus>('get_care_status');
      patchStreamState({ careStatus: status });
    } catch {
      // Offline is a normal early state; the section simply stays idle.
      patchStreamState({ careStatus: null });
    }
  });
}

/** Opens the explicit consent dialog. Nothing runs until the owner confirms. */
export function openCareConsent(): void {
  careUi.update((state) => ({ ...state, consentDialogOpen: true }));
}

export function closeCareConsent(): void {
  careUi.update((state) => ({ ...state, consentDialogOpen: false }));
}

/**
 * Approves the plan shown for one run, then starts that run in the same user gesture. The
 * service approves the plan it composes now; if that is not the plan shown, nothing starts
 * and the new plan is shown for approval. A refused or failed call is reported by runBusy.
 */
export async function authorizeAndStartCare(): Promise<void> {
  const shown = get(streamState).careStatus;
  closeCareConsent();
  await runBusy(async () => {
    const granted = await serviceInvoke<CareRunStatus>('grant_care_session_consent');
    const next = afterApproval(shown, granted);
    if (!next.start) {
      patchStreamState({ careStatus: next.status });
      return;
    }
    const status = await serviceInvoke<CareRunStatus>('start_care_run');
    patchStreamState({ careStatus: status });
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
