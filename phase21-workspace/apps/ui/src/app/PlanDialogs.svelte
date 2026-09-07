<script lang="ts">
  import { fluidPress } from '../design/motion';
  import { shellState } from './shell-state';
  import { streamState } from '../platform/stream-state';
  import { FluidDialog, Pressable, TechnicalText } from '../design/primitives';
  import { localizeDirection, localizeRisk, localizeState, t, tp } from '../lib/i18n';
  import { authorizeAndInstall, closeDriverReview, driversUi } from '../features/drivers/controller';
  import { authorizeAndRepair, closeRepairReview, repairUi } from '../features/repair/controller';
  import { authorizeAndCleanup, cleanupUi, closeCleanupReview } from '../features/cleanup/controller';
  import { authorizeAndStartCare, careUi, closeCareConsent } from '../features/care/controller';
  import {
    authorizeAndApplyStartup,
    authorizeAndRestoreStartup,
    closeStartupRestore,
    finalizeStartupRestore,
    closeStartupReview,
    prepareStartupRestore,
    selectedStartupServices,
    setRestoreServiceConfirmed,
    startupUi,
  } from '../features/startup/controller';
  import { formatBytes, shortDigest } from '../features/shared';

  $: busy = $shellState.busy;
  $: locale = $shellState.locale;
  $: installPlan = $streamState.installPlan;
  $: repairPlan = $streamState.repairPlan;
  $: repairIntelligence = $streamState.repairAssessment.intelligence;
  const runtimeExecutableActions = new Set(['repairComponentStore','repairSystemFiles','startRequiredService']);
  $: repairDialogNodes = repairIntelligence?.graph.nodes.some((node)=>node.action==='reboot' || node.rebootBoundaryAfter) ? [] : (repairIntelligence?.graph.nodes.filter((node)=>node.executableAutomatically && runtimeExecutableActions.has(node.action)) ?? []);
  $: cleanupPlan = $streamState.cleanupPlan;
  $: startupPlan = $streamState.startupPlan;
  $: restoreEntry = $startupUi.restoreEntry;
  $: restorePlan = $startupUi.restorePlan;
</script>

{#if installPlan}
  <FluidDialog open={$driversUi.reviewOpen} labelledBy="install-review-title" onClose={closeDriverReview} className="review-dialog">
    <p class="eyebrow">{t('dialog.driver.eyebrow',locale)}</p>
    <h2 id="install-review-title">{t('dialog.driver.title',locale)}</h2>
    <p class="review-copy">{t('dialog.driver.copy',locale)}</p>
    <div class="review-facts">
      <div><span>{t('dialog.actions',locale)}</span><strong>{tp('unit.deviceAction',locale,installPlan.actionCount)}</strong></div>
      <div><span>{t('common.risk',locale)}</span><strong>{localizeRisk(installPlan.risk,locale)}</strong></div>
      <div><span>{t('dialog.inventoryEpoch',locale)}</span><TechnicalText value={String(installPlan.inventoryEpoch)}/></div>
      <div><span>{t('common.planDigest',locale)}</span><TechnicalText value={shortDigest(installPlan.digest)} as="code"/></div>
    </div>
    <div class="review-warning"><strong>{t('dialog.adminRequired',locale)}</strong><p>{t('dialog.driver.warning',locale)}</p></div>
    <div class="review-actions">
      <Pressable className="secondary" onclick={closeDriverReview} disabled={busy}>{t('common.cancel',locale)}</Pressable>
      <Pressable className="primary" onclick={authorizeAndInstall} disabled={busy}>{busy ? t('dialog.waitingConsent',locale) : t('dialog.authorizeInstall',locale)}</Pressable>
    </div>
  </FluidDialog>
{/if}

{#if repairPlan}
  <FluidDialog open={$repairUi.reviewOpen} labelledBy="repair-review-title" onClose={closeRepairReview} className="review-dialog">
    <p class="eyebrow">{t('dialog.repair.eyebrow',locale)}</p>
    <h2 id="repair-review-title">{t('dialog.repair.title',locale)}</h2>
    <p class="review-copy">{t('dialog.repair.copy',locale)}</p>
    <div class="review-facts">
      <div><span>{t('dialog.assessment',locale)}</span><TechnicalText value={`${repairPlan.scanId.slice(0,12)}…`}/></div>
      <div><span>{t('common.risk',locale)}</span><strong>{localizeRisk(repairPlan.risk,locale)}</strong></div>
      <div><span>{t('dialog.diskScan',locale)}</span><strong>{$repairUi.planDiskScan === null ? t('common.frozenInService',locale) : ($repairUi.planDiskScan ? t('dialog.onlineScan',locale) : t('dialog.notSelected',locale))}</strong></div>
      <div><span>{t('common.planDigest',locale)}</span><TechnicalText value={shortDigest(repairPlan.digest)} as="code"/></div>
    </div>
    {#if repairIntelligence}
      <div class="repair-dialog-plan" aria-label={t('repair.planEyebrow',locale)}>
        {#each repairDialogNodes as node (node.id)}
          <div><span>✓</span><strong>{t(`repair.action.${node.action}` as never,locale)}</strong><small>{t(`repair.safety.${node.safety}` as never,locale)}</small></div>
        {/each}
      </div>
      <div class="review-facts">
        <div><span>{t('repair.systemRestore',locale)}</span><strong>{localizeState(repairIntelligence.recovery.systemRestore,locale)}</strong></div>
        <div><span>{t('repair.winre',locale)}</span><strong>{localizeState(repairIntelligence.recovery.winRe,locale)}</strong></div>
        <div><span>{t('repair.restart',locale)}</span><strong>{repairIntelligence.graph.nodes.some((node)=>node.rebootBoundaryAfter) ? t('repair.restartRequired',locale) : t('repair.noRestartBarrier',locale)}</strong></div>
        <div><span>{t('common.planDigest',locale)}</span><TechnicalText value={shortDigest(repairIntelligence.graph.digestSha256)} as="code"/></div>
      </div>
    {/if}
    <div class="review-warning"><strong>{t('dialog.adminRequired',locale)}</strong><p>{t('dialog.repair.warning',locale)}</p></div>
    <div class="review-actions">
      <Pressable className="secondary" onclick={closeRepairReview} disabled={busy}>{t('common.cancel',locale)}</Pressable>
      <Pressable className="primary" onclick={authorizeAndRepair} disabled={busy}>{busy ? t('dialog.waitingConsent',locale) : t('dialog.authorizeRepair',locale)}</Pressable>
    </div>
  </FluidDialog>
{/if}

{#if startupPlan}
  <FluidDialog open={$startupUi.reviewOpen} labelledBy="startup-review-title" onClose={closeStartupReview} className="review-dialog">
    <p class="eyebrow">{t('dialog.startup.eyebrow',locale)}</p>
    <h2 id="startup-review-title">{t('dialog.startup.title',locale)}</h2>
    <p class="review-copy">{t('dialog.startup.copy',locale)}</p>
    <div class="review-facts">
      <div><span>{t('dialog.explicitActions',locale)}</span><strong>{tp('unit.action',locale,startupPlan.actionCount)}</strong></div>
      <div><span>{t('common.risk',locale)}</span><strong>{localizeRisk(startupPlan.risk,locale)}</strong></div>
      <div><span>{t('dialog.services',locale)}</span><strong>{tp('unit.serviceTarget',locale,selectedStartupServices().length)}</strong></div>
      <div><span>{t('common.planDigest',locale)}</span><TechnicalText value={shortDigest(startupPlan.digest)} as="code"/></div>
    </div>
    <div class="review-warning"><strong>{t('dialog.adminRequired',locale)}</strong><p>{t('dialog.startup.warning',locale)}</p></div>
    <div class="review-actions">
      <Pressable className="secondary" onclick={closeStartupReview} disabled={busy}>{t('common.cancel',locale)}</Pressable>
      <Pressable className="primary" onclick={authorizeAndApplyStartup} disabled={busy}>{busy ? t('dialog.waitingConsent',locale) : t('dialog.authorizeApply',locale)}</Pressable>
    </div>
  </FluidDialog>
{/if}

{#if restoreEntry}
  <FluidDialog open={$startupUi.restoreOpen} labelledBy="startup-restore-title" onClose={closeStartupRestore} onClosed={finalizeStartupRestore} className="review-dialog">
    <p class="eyebrow">{t('dialog.restore.eyebrow',locale)}</p>
    <h2 id="startup-restore-title">{t('dialog.restore.title',locale,{name:restoreEntry.displayName})}</h2>
    <p class="review-copy">{t('dialog.restore.copy',locale)}</p>
    {#if restoreEntry.kind === 'Service'}
      <label use:fluidPress={{ pressedScale:0.992 }} class="service-confirm modal-confirm"><input type="checkbox" checked={$startupUi.restoreServiceConfirmed} onchange={(event) => setRestoreServiceConfirmed((event.currentTarget as HTMLInputElement).checked)}/><span></span><div><strong>{t('dialog.restore.serviceTitle',locale)}</strong><p>{t('dialog.restore.serviceCopy',locale)}</p></div></label>
    {/if}
    {#if restorePlan}
      <div class="review-facts">
        <div><span>{t('dialog.direction',locale)}</span><strong>{localizeDirection('Restore',locale)}</strong></div>
        <div><span>{t('common.risk',locale)}</span><strong>{localizeRisk(restorePlan.risk,locale)}</strong></div>
        <div><span>{t('dialog.originalChange',locale)}</span><TechnicalText value={`${restoreEntry.changeId.slice(0,12)}…`}/></div>
        <div><span>{t('common.planDigest',locale)}</span><TechnicalText value={shortDigest(restorePlan.digest)} as="code"/></div>
      </div>
    {/if}
    <div class="review-actions">
      <Pressable className="secondary" onclick={closeStartupRestore} disabled={busy}>{t('common.cancel',locale)}</Pressable>
      {#if restorePlan}
        <Pressable className="primary" onclick={authorizeAndRestoreStartup} disabled={busy}>{busy ? t('dialog.waitingConsent',locale) : t('dialog.authorizeRestore',locale)}</Pressable>
      {:else}
        <Pressable className="primary" onclick={prepareStartupRestore} disabled={busy || (restoreEntry.kind === 'Service' && !$startupUi.restoreServiceConfirmed)}>{t('dialog.createRestore',locale)}</Pressable>
      {/if}
    </div>
  </FluidDialog>
{/if}

{#if cleanupPlan}
  <FluidDialog open={$cleanupUi.reviewOpen} labelledBy="cleanup-review-title" onClose={closeCleanupReview} className="review-dialog">
    <p class="eyebrow">{t('dialog.cleanup.eyebrow',locale)}</p>
    <h2 id="cleanup-review-title">{t('dialog.cleanup.title',locale)}</h2>
    <p class="review-copy">{t('dialog.cleanup.copy',locale)}</p>
    <div class="review-facts">
      <div><span>{t('dialog.categories',locale)}</span><strong>{tp('unit.category',locale,cleanupPlan.actionCount)}</strong></div>
      <div><span>{t('dialog.expectedSpace',locale)}</span><strong>{$cleanupUi.planExpectedBytes ? formatBytes($cleanupUi.planExpectedBytes,locale) : t('common.frozenInService',locale)}</strong></div>
      <div><span>{t('dialog.explicitCategories',locale)}</span><strong>{$cleanupUi.planExplicit === null ? t('common.frozenInService',locale) : ($cleanupUi.planExplicit ? t('dialog.includedByYou',locale) : t('common.none',locale))}</strong></div>
      <div><span>{t('common.planDigest',locale)}</span><TechnicalText value={shortDigest(cleanupPlan.digest)} as="code"/></div>
    </div>
    <div class="review-warning"><strong>{t('dialog.cleanup.warningTitle',locale)}</strong><p>{t('dialog.cleanup.warning',locale)}</p></div>
    <div class="review-actions">
      <Pressable className="secondary" onclick={closeCleanupReview} disabled={busy}>{t('common.cancel',locale)}</Pressable>
      <Pressable className="primary" onclick={authorizeAndCleanup} disabled={busy}>{busy ? t('dialog.waitingConsent',locale) : t('dialog.authorizeClean',locale)}</Pressable>
    </div>
  </FluidDialog>
{/if}

<FluidDialog open={$careUi.consentDialogOpen} labelledBy="care-consent-title" onClose={closeCareConsent} className="review-dialog">
  <p class="eyebrow">{t('care.eyebrow', locale)}</p>
  <h2 id="care-consent-title">{t('care.consentTitle', locale)}</h2>
  <p class="review-copy">{t('care.consentCopy', locale)}</p>
  <ul class="consent-list">
    <li>{t('care.consentBulletScope', locale)}</li>
    <li>{t('care.consentBulletReview', locale)}</li>
    <li>{t('care.consentBulletSession', locale)}</li>
  </ul>
  <div class="review-actions">
    <Pressable className="secondary" onclick={closeCareConsent} disabled={busy}>{t('common.cancel', locale)}</Pressable>
    <Pressable className="primary" onclick={authorizeAndStartCare} disabled={busy}>{busy ? t('dialog.waitingConsent', locale) : t('care.consentAuthorize', locale)}</Pressable>
  </div>
</FluidDialog>
