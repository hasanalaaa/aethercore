import { uiTransport } from './transport';

export async function serviceInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return uiTransport.invoke<T>(command, args);
}
