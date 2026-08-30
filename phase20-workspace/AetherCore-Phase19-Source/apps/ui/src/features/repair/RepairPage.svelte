<script lang="ts">
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { LocalizedOwnedText, Pressable, ProgressBar, TechnicalText } from '../../design/primitives';
  import { localizeOwnedText, localizeState, t } from '../../lib/i18n';
  import { openRepairReview, repairActive, repairUi, reviewSystemRepair, setIncludeDiskScan, startRepairAssessment } from './controller';
  import { shortDigest, stageTone } from '../shared';

  $: snapshot = $streamState.snapshot;
  $: repairAssessment = $streamState.repairAssessment;
  $: intelligence = repairAssessment.intelligence;
  $: repairPlan = $streamState.repairPlan;
  $: repairStatus = $streamState.repairStatus;
  $: busy = $shellState.busy;
  $: locale = $shellState.locale;
  $: includeDiskScan = $repairUi.includeDiskScan;
  $: recommendedNodes = intelligence?.graph.nodes.filter((node) => !['level0Diagnostic'].includes(node.safety)) ?? [];
  const runtimeExecutableActions = new Set(['repairComponentStore','repairSystemFiles','startRequiredService']);
  $: hasRebootBarrier = intelligence?.graph.nodes.some((node) => node.action === 'reboot' || node.rebootBoundaryAfter) ?? false;
  $: executableNodes = hasRebootBarrier ? [] : recommendedNodes.filter((node) => node.executableAutomatically && runtimeExecutableActions.has(node.action));
  $: recovery = intelligence?.recovery;
  $: healthyFacts = intelligence?.facts.filter((fact) => ['healthy','available'].includes(fact.state)).length ?? 0;
  $: attentionFacts = intelligence?.facts.filter((fact) => !['healthy','available','unknown'].includes(fact.state)).length ?? 0;

  const domains = ['componentStore','systemFiles','servicing','windowsUpdate','network','dns','proxy','filesystem','recovery'];
  function domainState(domain:string): string {
    const facts = intelligence?.facts.filter((fact) => fact.domain === domain) ?? [];
    if (!facts.length) return 'unknown';
    if (facts.some((fact) => !['healthy','available','unknown'].includes(fact.state))) return 'attention';
    if (facts.every((fact) => ['healthy','available'].includes(fact.state))) return 'healthy';
    return 'unknown';
  }
  function diagnosisFor(domain:string) { return intelligence?.diagnoses.find((item) => item.domain === domain); }
</script>

<header>
  <div>
    <p class="eyebrow">{t('repair.eyebrow', locale)}</p>
    <h1>{t('repair.title', locale)}</h1>
    <p class="sub">{t('repair.subtitle', locale)}</p>
  </div>
  <div class="header-actions">
    <div class="service-pill"><span class:online={snapshot.connected}></span>{snapshot.connected ? t('common.engineOnline', locale, { version: snapshot.serviceVersion }) : t('common.engineOffline', locale)}</div>
    <Pressable className="scan-button" onclick={startRepairAssessment} disabled={busy || repairAssessment.state === 'Scanning' || repairActive() || !snapshot.connected}>
      <span>◇</span>{repairAssessment.state === 'Scanning' ? t('repair.assessing', locale) : t('repair.scanWindows', locale)}
    </Pressable>
  </div>
</header>

<section class="phase4-hero windows-health-hero" aria-live="polite">
  <div class="phase4-hero-copy">
    <div class:ready={repairAssessment.state === 'Ready' && attentionFacts === 0} class:scanning={repairAssessment.state === 'Scanning'} class:attention={attentionFacts > 0} class="scan-orb"><span>{repairAssessment.state === 'Ready' ? (attentionFacts ? '!' : '✓') : repairAssessment.state === 'Failed' ? '!' : '◇'}</span></div>
    <div>
      <p class="eyebrow">{t('repair.windowsHealth', locale)}</p>
      <h2>{repairAssessment.state === 'Idle' ? t('repair.hero.idle', locale) : repairAssessment.state === 'Scanning' ? t('repair.hero.scanning', locale) : repairAssessment.state === 'Ready' ? (attentionFacts ? t('repair.hero.attention', locale) : t('repair.hero.healthy', locale)) : t('repair.hero.failed', locale)}</h2>
      {#if repairAssessment.errorMessage}
        <LocalizedOwnedText value={repairAssessment.errorMessage} {locale} as="p"/>
      {:else if repairAssessment.state === 'Ready'}
        <p>{attentionFacts ? t('repair.health.attentionCopy', locale, { count: attentionFacts }) : t('repair.health.healthyCopy', locale)}</p>
      {:else}
        <p>{t('repair.assessmentCopy', locale)}</p>
      {/if}
    </div>
  </div>
  {#if repairAssessment.state === 'Scanning'}<div class="indeterminate"><span></span></div>{/if}
</section>

{#if intelligence}
  <section class="repair-health-grid" aria-label={t('repair.healthGrid', locale)}>
    {#each domains as domain}
      <article class:healthy={domainState(domain)==='healthy'} class:attention={domainState(domain)==='attention'}>
        <span class="health-state-dot"></span>
        <div><small>{t(`repair.domain.${domain}` as never, locale)}</small><strong>{t(`repair.health.${domainState(domain)}` as never, locale)}</strong></div>
        {#if diagnosisFor(domain)}<TechnicalText value={diagnosisFor(domain)?.code ?? ''} as="code"/>{/if}
      </article>
    {/each}
  </section>

  {#if intelligence.diagnoses.length}
    <section class="repair-diagnosis-panel">
      <div class="section-heading"><div><p class="eyebrow">{t('repair.diagnosisEyebrow',locale)}</p><h2>{t('repair.diagnosisTitle',locale)}</h2></div><span>{t('repair.evidenceCount',locale,{count:intelligence.facts.length})}</span></div>
      <div class="repair-diagnosis-list">
        {#each intelligence.diagnoses as diagnosis (diagnosis.id)}
          <article>
            <div><span class="role-chip">{t(`repair.role.${diagnosis.role}` as never,locale)}</span><strong>{t(`repair.diagnosis.${diagnosis.code}` as never,locale)}</strong></div>
            <p>{t(`repair.diagnosisCopy.${diagnosis.code}` as never,locale)}</p>
            <details><summary>{t('repair.technicalDetails',locale)}</summary><TechnicalText value={`${diagnosis.code} · ${diagnosis.confidence} · ${diagnosis.ruleVersion}`}/><LocalizedOwnedText value={diagnosis.uncertainty} {locale} as="p"/></details>
          </article>
        {/each}
      </div>
    </section>
  {/if}

  <section class="recovery-readiness-card">
    <div><p class="eyebrow">{t('repair.recoveryEyebrow',locale)}</p><h3>{t('repair.recoveryTitle',locale)}</h3><p>{t('repair.recoveryCopy',locale)}</p></div>
    <div class="recovery-facts">
      <div><span>{t('repair.systemRestore',locale)}</span><strong>{localizeState(recovery?.systemRestore ?? 'unknown',locale)}</strong></div>
      <div><span>{t('repair.restorePoint',locale)}</span><strong>{localizeState(recovery?.restorePointCreation ?? 'unknown',locale)}</strong></div>
      <div><span>{t('repair.winre',locale)}</span><strong>{localizeState(recovery?.winRe ?? 'unknown',locale)}</strong></div>
      <div><span>{t('repair.journalRecovery',locale)}</span><strong>{localizeState(recovery?.journalRecovery ?? 'unknown',locale)}</strong></div>
    </div>
  </section>
{/if}

{#if repairAssessment.checks.length}
  <details class="repair-evidence-disclosure">
    <summary>{t('repair.evidenceDisclosure',locale)} · {repairAssessment.checks.length}</summary>
    <section class="repair-check-grid">
      {#each repairAssessment.checks as check (check.id)}
        <article class:bad={check.stage === 'Attention' || check.stage === 'Critical'}>
          <div class="check-icon">{check.stage === 'Unknown' ? '?' : (check.stage === 'Attention' || check.stage === 'Critical') ? '!' : '✓'}</div>
          <div><span>{localizeState(check.stage, locale)}</span><LocalizedOwnedText value={check.title} {locale} as="strong"/><LocalizedOwnedText value={check.detail} {locale} as="p"/>{#if check.logHint}<TechnicalText value={check.logHint}/>{/if}</div>
        </article>
      {/each}
    </section>
  </details>
{/if}

{#if repairAssessment.state === 'Ready' && recommendedNodes.length === 0 && !repairActive()}
  <section class="phase4-action-card healthy-repair-card"><div><p class="eyebrow">{t('repair.noRepairEyebrow',locale)}</p><h3>{t('repair.noRepairTitle',locale)}</h3><p>{t('repair.noRepairCopy',locale,{healthy:healthyFacts})}</p></div></section>
{:else if repairAssessment.state === 'Ready' && executableNodes.length > 0 && !repairActive() && !(repairPlan && repairPlan.state === 'AwaitingAuthorization')}
  <section class="phase4-action-card">
    <div><p class="eyebrow">{t('repair.planEyebrow', locale)}</p><h3>{t('repair.recommendedTitle', locale,{count:executableNodes.length})}</h3><p>{t('repair.planCopy', locale)}</p></div>
    <div class="repair-plan-preview">
      {#each executableNodes as node (node.id)}<div><span>✓</span><strong>{t(`repair.action.${node.action}` as never,locale)}</strong><small>{t(`repair.safety.${node.safety}` as never,locale)}</small></div>{/each}
    </div>
    {#if repairAssessment.checks.some((check)=>check.id==='disk-scan' && check.resultCode!=='NoErrors')}
      <label use:fluidPress={{ pressedScale: 0.992 }} class="safe-toggle"><input type="checkbox" checked={includeDiskScan} onchange={(event) => setIncludeDiskScan((event.currentTarget as HTMLInputElement).checked)}/><span></span><div><strong>{t('repair.includeDisk', locale)}</strong><small>{t('repair.diskHint', locale)}</small></div></label>
    {/if}
    <Pressable className="install-button" onclick={reviewSystemRepair} disabled={busy}>{t('repair.review', locale)}</Pressable>
  </section>
{:else if repairAssessment.state === 'Ready' && recommendedNodes.length > 0 && executableNodes.length === 0 && !repairActive()}
  <section class="phase4-action-card recovery-escalation-card"><div><p class="eyebrow">{t('repair.escalationEyebrow',locale)}</p><h3>{t('repair.escalationTitle',locale)}</h3><p>{t('repair.escalationCopy',locale)}</p></div></section>
{/if}

{#if repairPlan && !repairStatus && repairPlan.state === 'AwaitingAuthorization'}
  <section class="pending-install-card"><div><p class="eyebrow">{t('repair.immutablePlan', locale)}</p><h3>{t('repair.waiting', locale)}</h3><p>{t('repair.planRef', locale, { assessment: repairPlan.scanId.slice(0, 8), digest: shortDigest(repairPlan.digest) })}</p></div><Pressable className="primary" onclick={openRepairReview} disabled={busy}>{t('common.reviewAuthorize', locale)}</Pressable></section>
{/if}

{#if repairStatus}
  <section class:terminal={['Completed','Failed'].includes(repairStatus.planState)} class="install-progress-card" aria-live="polite">
    <div class="install-progress-head"><div><p class="eyebrow">{t('repair.execution', locale)}</p><h3>{localizeState(repairStatus.stage, locale)}</h3>{#if repairStatus.detail}<LocalizedOwnedText value={repairStatus.detail} {locale} as="p"/>{/if}</div><span class:bad={stageTone(repairStatus.stage)==='bad'} class="execution-state">{localizeState(repairStatus.planState, locale)}</span></div>
    {#if repairStatus.progressKnown}<ProgressBar value={repairStatus.overallPercent} label={t('repair.progressLabel', locale)} />{:else if repairActive()}<ProgressBar value={0} known={false} label={t('repair.progressUnknown', locale)} />{/if}
    <div class="phase4-safety-row"><div class:ready={!repairStatus.mutationStarted}><span>{t('repair.before', locale)}</span><strong>{repairStatus.mutationStarted ? t('repair.checkpointCrossed', locale) : t('repair.noMutation', locale)}</strong></div><div class:ready={repairStatus.verificationState==='Verified'}><span>{t('repair.verification',locale)}</span><strong>{localizeState(repairStatus.verificationState || 'Pending',locale)}</strong></div><div class:ready={!repairStatus.rebootRequired}><span>{t('repair.restart',locale)}</span><strong>{repairStatus.rebootRequired ? t('repair.restartRequired',locale) : t('repair.noRestartBarrier',locale)}</strong></div></div>
    {#if repairStatus.failureMessage}<div class="install-failure"><strong>{t('repair.failure', locale)}</strong><LocalizedOwnedText value={repairStatus.failureMessage} {locale} as="p"/></div>{/if}
    {#if repairStatus.steps.length}<div class="phase4-step-list">{#each repairStatus.steps as step (step.id)}<div><span class="step-dot"></span><div><LocalizedOwnedText value={step.title} {locale} as="strong"/><LocalizedOwnedText value={step.detail} {locale} as="p"/></div><em><TechnicalText value={step.resultCode || step.stage}/></em></div>{/each}</div>{/if}
  </section>
{/if}

<section class="safety-note"><span>◇</span><div><strong>{t('repair.safetyTitle', locale)}</strong><p>{t('repair.safetyCopy', locale)}</p></div></section>
