<script lang="ts">
  import { shellState } from '../../app/shell-state';
  import { fluidPress } from '../../design/motion';
  import ProgressBar from '../../design/primitives/ProgressBar.svelte';
  import TechnicalText from '../../design/primitives/TechnicalText.svelte';
  import { hasMessageKey, t, td } from '../../lib/i18n';
  import { formatBytes } from '../shared';
  import { streamState } from '../../platform/stream-state';
  import { checkUpdates, exportSupportBundle, installStagedUpdate, previewSupportBundle, setUpdateChannel, stageLatestUpdate, systemCareUi } from './controller';
  $: locale=$shellState.locale;
  $: update=$streamState.updateSnapshot;
  $: preview=$systemCareUi.supportPreview;
  $: busy=$shellState.busy;
  $: redactions=preview?.privacy ? preview.privacy.userPathRedactions+preview.privacy.accountIdentifierRedactions+preview.privacy.hardwareSerialRedactions+preview.privacy.emailRedactions : 0;
  function msg(key:string,fallback:'update.status.unknown'|'support.section.unknown'):string{return hasMessageKey(key)?td(key,locale):t(fallback,locale)}
</script>

<section class="system-care-grid" aria-label={t('systemCare.title',locale)}>
  <article class="panel system-care-card">
    <!-- "Installation still requires administrator approval" is the policy
         band's promise; the status line below states what the updater found. -->
    <div class="panel-head"><div><h3>{t('update.title',locale)}</h3></div><span class="live-badge">{t('update.channelLabel',locale)}</span></div>
    <div class="segmented" role="group" aria-label={t('update.channelLabel',locale)}>
      <button use:fluidPress={{pressedScale:0.985}} class:active={$systemCareUi.channel===1} aria-pressed={$systemCareUi.channel===1} onclick={()=>setUpdateChannel(1)}>{t('update.channel.stable',locale)}</button>
      <button use:fluidPress={{pressedScale:0.985}} class:active={$systemCareUi.channel===2} aria-pressed={$systemCareUi.channel===2} onclick={()=>setUpdateChannel(2)}>{t('update.channel.beta',locale)}</button>
    </div>
    <div class="system-care-status"><strong>{msg(update.statusMessageKey,'update.status.unknown')}</strong><span>{t('update.currentVersion',locale,{version:update.currentVersion})}</span></div>
    {#if update.latestRelease}
      <div class="release-row"><div><span>{t('update.latestVersion',locale)}</span><TechnicalText value={update.latestRelease.version}/></div><div><span>{t('update.size',locale)}</span><strong>{formatBytes(update.latestRelease.sizeBytes,locale)}</strong></div></div>
    {/if}
    {#if update.progressKnown}<ProgressBar value={update.overallPercent} label={t('update.downloadProgress',locale)}/>{/if}
    <div class="actions">
      <button use:fluidPress={{pressedScale:0.985}} class="secondary" disabled={busy||update.state===3||update.state===6} onclick={checkUpdates}>{t('update.check',locale)}</button>
      {#if update.state===5 && update.latestRelease}<button use:fluidPress={{pressedScale:0.985}} class="primary" disabled={busy} onclick={stageLatestUpdate}>{t('update.download',locale)}</button>{/if}
      {#if update.state===7 && update.stagedRelease}<button use:fluidPress={{pressedScale:0.985}} class="primary" disabled={busy} onclick={installStagedUpdate}>{t('update.install',locale)}</button>{/if}
    </div>
  </article>

  <article class="panel system-care-card">
    <!-- KEPT, cut to the fact. A person decides whether to export before a
         preview exists, and what is NOT in the bundle is the part of that
         decision the screen cannot otherwise show. -->
    <div class="panel-head"><div><h3>{t('support.title',locale)}</h3></div><span class="live-badge">{t('support.privacyFirst',locale)}</span></div>
    <p>{t('support.copy',locale)}</p>
    {#if preview}
      <div class="support-preview" aria-live="polite">
        <strong>{t('support.previewReady',locale,{count:preview.sections.length})}</strong>
        <span>{t('support.redactions',locale,{count:redactions})}</span>
        <span>{t('support.estimatedSize',locale,{size:formatBytes(preview.estimatedSizeBytes,locale)})}</span>
        <ul>{#each preview.sections as section}<li><span>{msg(section.displayKey,'support.section.unknown')}</span><small>{formatBytes(section.sizeBytes,locale)}</small></li>{/each}</ul>
      </div>
    {/if}
    {#if $systemCareUi.exportResult}<div class="exported"><p><span>{t('support.exported',locale)}</span><TechnicalText value={$systemCareUi.exportResult.path}/></p><p><span>{t('support.verificationFingerprint',locale)}</span><TechnicalText value={$systemCareUi.exportResult.verificationFingerprintSha256}/></p></div>{/if}
    <div class="actions"><button use:fluidPress={{pressedScale:0.985}} class="secondary" disabled={busy} onclick={previewSupportBundle}>{t('support.inspect',locale)}</button>{#if preview}<button use:fluidPress={{pressedScale:0.985}} class="primary" disabled={busy} onclick={exportSupportBundle}>{t('support.export',locale)}</button>{/if}</div>
  </article>
</section>
