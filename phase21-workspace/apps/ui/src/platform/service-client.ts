import { serviceErrorKey } from '../lib/i18n';
import { uiTransport } from './transport';

export async function serviceInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await uiTransport.invoke<T>(command, args);
  } catch (error) {
    // The desktop rejects with a string: the service's message key, or the OS's own sentence.
    // An Error is the runtime's own fault and stays one (setError shows it as a generic failure).
    if (typeof error !== 'string') throw error;
    const key = serviceErrorKey(error);
    // The raw text goes to the local log only; what reaches the page is a key the catalogs can say.
    if (key !== error) console.error('[AetherCore] service call failed', command, error);
    throw key;
  }
}
