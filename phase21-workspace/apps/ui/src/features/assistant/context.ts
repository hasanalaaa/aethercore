import type { AssistantEvidenceRef } from '../../lib/contracts';

/** A contextual entry never promises evidence from another snapshot or surface. */
export function knownContext(pack: readonly AssistantEvidenceRef[], evidenceId: string, surface: string): boolean {
  return !!evidenceId && pack.some((item) => item.evidenceId === evidenceId && item.surface === surface);
}
