import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UiTransport } from './transport-contract';

type TestInjectableGlobal = typeof globalThis & {
  __AETHERCORE_TEST_TRANSPORT__?: UiTransport;
};

const tauriTransport: UiTransport = {
  invoke: <T>(command: string, args?: Record<string, unknown>) => invoke<T>(command, args),
  listen: <T>(event: string, handler: (event: { payload: T }) => void) => listen<T>(event, handler),
};

function injectedTransport(): UiTransport | undefined {
  if (import.meta.env.VITE_AETHERCORE_TEST_TRANSPORT !== '1') return undefined;
  const candidate = (globalThis as TestInjectableGlobal).__AETHERCORE_TEST_TRANSPORT__;
  if (!candidate || typeof candidate.invoke !== 'function' || typeof candidate.listen !== 'function') return undefined;
  return candidate;
}

/**
 * Resolve once during module initialization. The test transport must be installed by the browser
 * harness before the application module graph executes; production code cannot hot-swap the trust
 * boundary after startup.
 */
export const uiTransport: UiTransport = injectedTransport() ?? tauriTransport;
