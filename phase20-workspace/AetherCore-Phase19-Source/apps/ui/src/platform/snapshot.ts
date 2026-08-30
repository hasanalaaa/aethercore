import type { Snapshot } from '../lib/contracts';
import { serviceInvoke } from './service-client';
import { patchSnapshot } from './stream-state';

export async function refreshServiceSnapshot(): Promise<Snapshot> {
  const snapshot = await serviceInvoke<Snapshot>('get_snapshot');
  patchSnapshot(snapshot);
  return snapshot;
}
