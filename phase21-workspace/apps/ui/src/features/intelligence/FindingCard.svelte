<script lang="ts">
  import { MaterialSurface, TechnicalText } from '../../design/primitives';
  import { EvidenceChip, type Evidence } from '../../design/signature';
  import type { PcFinding } from '../../lib/contracts';
  import { formatDateTime, formatNumber, hasMessageKey, td, t, type Locale, type MessageKey } from '../../lib/i18n';

  export let finding: PcFinding;
  export let locale: Locale;
  /**
   * The observation this finding cites. Required: `DeepScanPage` runs findings
   * through `citedOnly` before rendering any of them, so a card is only ever
   * built for a finding that already has one.
   */
  export let evidence: Evidence;

  function message(key: string): string {
    if (!hasMessageKey(key)) return key;
    const args = Object.fromEntries(finding.messageArgs.map((entry) => [entry.key, entry.value]));
    return td(key, locale, args);
  }
  function semanticMessage(key: string): string { return hasMessageKey(key) ? td(key, locale) : key; }
  function severityLabel(value:number):string { const key:MessageKey=value===5?'deepScan.severity.critical':value===4?'deepScan.severity.high':value===3?'deepScan.severity.moderate':value===2?'deepScan.severity.low':'deepScan.severity.informational'; return td(key,locale); }
  function confidenceLabel(value:number):string { const key:MessageKey=value===5?'deepScan.confidence.confirmed':value===4?'deepScan.confidence.high':value===3?'deepScan.confidence.medium':value===2?'deepScan.confidence.low':'deepScan.confidence.unknown'; return td(key,locale); }
  function safetyLabel(value:number):string { const key:MessageKey=value===5?'deepScan.safety.hardware':value===4?'deepScan.safety.manual':value===3?'deepScan.safety.sensitive':value===2?'deepScan.safety.review':'deepScan.safety.auto'; return td(key,locale); }
  function rebootLabel(value:string):string { const key:MessageKey=value==='Required'?'deepScan.reboot.required':value==='Possible'?'deepScan.reboot.maybe':'deepScan.reboot.none'; return td(key,locale); }
  function reversibilityLabel(value:string):string { const key:MessageKey=value==='Reversible'?'deepScan.reversible.yes':value==='Checkpointed'?'deepScan.reversible.checkpointed':value==='Limited'?'deepScan.reversible.partial':value==='NotSoftwareReversible'?'deepScan.reversible.no':'deepScan.reversible.unknown'; return td(key,locale); }
  function verificationLabel(value:string):string|null {
    if (value === 'VerificationUnavailable') return td('deepScan.verification.unavailable',locale);
    if (value === 'NotRechecked') return td('deepScan.verification.notRechecked',locale);
    if (value === 'ResolutionConfirmed') return td('deepScan.verification.resolutionConfirmed',locale);
    return null;
  }
  function correlationStrength(value:string):string {
    const key:MessageKey=value==='Strong'?'deepScan.correlation.strong':value==='Moderate'?'deepScan.correlation.moderate':'deepScan.correlation.weak';
    return td(key,locale);
  }
</script>

<MaterialSurface level="structural" as="article" className={`finding-card severity-${finding.severity}`}>
  <div class="finding-head">
    <div class="severity-mark" aria-hidden="true">{finding.severity >= 4 ? '!' : finding.severity === 1 ? 'i' : '•'}</div>
    <div class="finding-copy">
      <div class="finding-meta"><span>{severityLabel(finding.severity)}</span><span>{t('deepScan.confidenceLabel',locale,{confidence:confidenceLabel(finding.confidence)})}</span></div>
      <h3>{message(finding.titleKey)}</h3>
      <p>{message(finding.summaryKey)}</p>
    </div>
    {#if finding.remediationAvailable}<span class="safety-badge">{safetyLabel(finding.remediationSafety)}</span>{/if}
  </div>
  {#if verificationLabel(finding.verificationStatus)}
    <p class="verification-note" role="status">{verificationLabel(finding.verificationStatus)}</p>
  {/if}
  <div class="finding-evidence">
    <EvidenceChip {evidence} {locale} />
  </div>
  <div class="finding-properties">
    <span>{t('deepScan.resource',locale,{resource:finding.affectedResource?.displayName ?? '—'})}</span>
    <span>{t('deepScan.reboot',locale,{value:rebootLabel(finding.rebootRequirement)})}</span>
    <span>{t('deepScan.reversible',locale,{value:reversibilityLabel(finding.reversibility)})}</span>
  </div>
  <details>
    <summary>{t('deepScan.why',locale)}</summary>
    <p>{message(finding.technicalKey)}</p>
    {#if finding.correlation}
      <div class="correlation-detail">
        <strong>{t('deepScan.correlation.strength',locale,{strength:correlationStrength(finding.correlation.strength)})}</strong>
        <span>{semanticMessage(finding.correlation.rationaleKey)}</span>
        <span>{t('deepScan.correlation.distance',locale,{seconds:formatNumber(Math.round(finding.correlation.timeDistanceMs/1000),locale)})}</span>
        {#if finding.correlation.sharedScope}<span>{t('deepScan.correlation.scope',locale,{scope:finding.correlation.sharedScope})}</span>{/if}
        {#each finding.correlation.conflictingEvidenceKeys as key}
          <span>{t('deepScan.correlation.uncertainty',locale,{detail:semanticMessage(key)})}</span>
        {/each}
      </div>
    {/if}
    <small>{t('deepScan.rule',locale,{rule:finding.ruleId,version:finding.ruleVersion})}</small>
  </details>
</MaterialSurface>

<style>
  :global(.finding-card){padding:16px 17px;border-radius:15px}
  .finding-head{display:grid;grid-template-columns:auto minmax(0,1fr) auto;gap:12px}.severity-mark{inline-size:30px;block-size:30px;border-radius:9px;display:grid;place-items:center;border:1px solid var(--ac-border-strong);font-weight:700}
  .finding-copy h3{margin:.2rem 0 .25rem;font-size:1rem;font-weight:590}.finding-copy p{margin:0;color:var(--ac-text-2);line-height:1.55}.finding-meta{display:flex;gap:10px;color:var(--ac-text-3);font-size:.76rem}.safety-badge{align-self:start;border:1px solid var(--ac-border-subtle);border-radius:999px;padding:.3rem .55rem;font-size:.74rem;color:var(--ac-text-2)}
  .verification-note{margin:12px 0 0;padding:9px 11px;border-radius:10px;border:1px solid var(--ac-border-subtle);background:var(--ac-material-base);color:var(--ac-text-2);font-size:.8rem;line-height:1.45}
  .finding-properties{display:flex;flex-wrap:wrap;gap:12px;margin:12px 0 0;padding-top:10px;border-top:1px solid var(--ac-border-subtle);color:var(--ac-text-3);font-size:.76rem}:global(.finding-card details){margin-top:10px}:global(.finding-card summary){cursor:pointer;color:var(--ac-text-2);font-weight:600}:global(.finding-card details>p){color:var(--ac-text-2)}
  .correlation-detail{display:grid;gap:5px;margin:9px 0;padding:10px 11px;border-radius:10px;background:var(--ac-material-base);color:var(--ac-text-2);font-size:.78rem}.correlation-detail strong{font-weight:650;color:var(--ac-text-1)}
  .finding-evidence{margin:12px 0 0}
  :global(.finding-card small){color:var(--ac-text-3)}
  @media (prefers-contrast:more){.severity-mark,.safety-badge,.verification-note{border-width:2px}}
</style>
