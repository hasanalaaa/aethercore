import { writable } from 'svelte/store';
import type { CareRunStatus } from '../../lib/contracts';
import { runBusy, setPage } from '../../app/shell-state';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState } from '../../platform/stream-state';

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
 * Grants one-time session consent, then starts the run in the same user gesture.
 * The service rejects the run if consent was not granted first.
 */
export async function authorizeAndStartCare(): Promise<void> {
  closeCareConsent();
  await runBusy(async () => {
    try {
      await serviceInvoke<CareRunStatus>('grant_care_session_consent');
      const status = await serviceInvoke<CareRunStatus>('start_care_run');
      patchStreamState({ careStatus: status });
    } catch {
      // Start failures surface through the returned status (Failed + evidence).
    }
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
