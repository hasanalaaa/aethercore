import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UiTransport } from './transport-contract';
import { hasTauriInternals } from './transport-env';

type TestInjectableGlobal = typeof globalThis & {
  __AETHERCORE_TEST_TRANSPORT__?: UiTransport;
};

const tauriTransport: UiTransport = {
  invoke: <T>(command: string, args?: Record<string, unknown>) => invoke<T>(command, args),
  listen: <T>(event: string, handler: (event: { payload: T }) => void) => listen<T>(event, handler),
};

export const tauriAvailable = typeof window !== 'undefined' && hasTauriInternals(window);

const disconnectedTransport: UiTransport = {
  invoke: async () => { throw new Error('AetherCore maintenance service is unavailable outside the desktop runtime.'); },
  listen: async () => { throw new Error('AetherCore maintenance service is unavailable outside the desktop runtime.'); },
};

function injectedTransport(): UiTransport | undefined {
  if (import.meta.env.VITE_AETHERCORE_TEST_TRANSPORT !== '1') return undefined;
  const candidate = (globalThis as TestInjectableGlobal).__AETHERCORE_TEST_TRANSPORT__;
  if (!candidate || typeof candidate.invoke !== 'function' || typeof candidate.listen !== 'function') return undefined;
  return candidate;
}

const testTransport = injectedTransport();

/**
 * Resolve once during module initialization. The test transport must be installed by the browser
 * harness before the application module graph executes; production code cannot hot-swap the trust
 * boundary after startup.
 */
export const transportAvailable = Boolean(testTransport || tauriAvailable);
export const uiTransport: UiTransport = testTransport ?? (tauriAvailable ? tauriTransport : disconnectedTransport);
