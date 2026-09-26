import type { MessageKey } from './i18n';

/**
 * The rules AetherCore refuses under, and the single place they are written down.
 *
 * These are constitutional, not runtime state: the product opens no network
 * connection the user did not start, keeps residency on the device, and installs
 * no unsigned driver. P75: the band used to say "Denied by policy: Outbound
 * network", which was false — an update check and Fleet do connect, on the
 * user's action. What is refused is network use at rest (no-egress lane: the
 * passive driver scan reads the local cache only).
 * They are always in force, which is why the policy band is persistent rather
 * than something that appears after a refusal. A user should learn what the
 * product will not do before they ask it to.
 *
 * The rule id is the contract with the service and the support bundle; it is
 * never localized, and it is always shown beside the refusal so a refusal can be
 * looked up rather than guessed at.
 */
export type PolicyRule = {
  /** Stable id, shown verbatim in monospace. Never translated. */
  id: string;
  /** What this rule refuses, in the user's language. */
  labelKey: MessageKey;
};

export const POLICY_RULES: readonly PolicyRule[] = [
  { id: 'NET-NO-EGRESS', labelKey: 'policy.outboundNetwork' },
  { id: 'RES-LOCAL-ONLY', labelKey: 'policy.cloudBackup' },
  { id: 'DRV-SIGNED-ONLY', labelKey: 'policy.unsignedDrivers' },
] as const;
