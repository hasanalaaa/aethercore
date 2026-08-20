import type { UiTransportUnlisten } from './transport-contract';
import { uiTransport } from './transport';
import type { UiKernelEvent, UiSessionState, UiStreamReset } from '../lib/contracts';
import { applyKernelEvent, applySessionState, applyStreamReset } from './stream-state';
import { announce, currentShellState, setError } from '../app/shell-state';
import { t } from '../lib/i18n';

export type KernelSessionCleanup = () => void;

export async function startKernelSession(): Promise<KernelSessionCleanup> {
  let disposed = false;
  const unlisteners: UiTransportUnlisten[] = [];

  try {
    unlisteners.push(await uiTransport.listen<UiKernelEvent>('aethercore://kernel-event', ({ payload }) => {
      if (!disposed) applyKernelEvent(payload);
    }));
    unlisteners.push(await uiTransport.listen<UiSessionState>('aethercore://session-state', ({ payload }) => {
      if (!disposed) applySessionState(payload);
    }));
    unlisteners.push(await uiTransport.listen<UiStreamReset>('aethercore://stream-reset', ({ payload }) => {
      if (disposed) return;
      applyStreamReset(payload);
      announce(t('announce.streamResynchronized', currentShellState().locale));
    }));

    const state = await uiTransport.invoke<UiSessionState>('start_ipc_session');
    if (!disposed) applySessionState(state);
  } catch (error) {
    if (!disposed) setError(error);
  }

  return () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
