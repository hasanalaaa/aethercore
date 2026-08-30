<script lang="ts">
  import { fluidPress } from '../../design/motion';
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { ProgressBar, Pressable, TechnicalText } from '../../design/primitives';
  import {
    authorizeDriverPlan, driverFilters, driversUi, filteredDevices, installActive, installTerminal,
    clearDriverSelection, openGpuSupport, reviewDriverInstall, scanStateIndex, scanStates, selectAllRecommended, selectedCandidates, selectedUpdates, setDriverCandidatePolicy, setDriverFilter, setDriverSearch,
    startDriverInstall, startDriverScan as startScan, toggleCandidate, toggleExpanded
  } from './controller';
  import { formatBytes, formatRange, shortDigest, stageTone, stateLabel, targetEvidence, targetLabel } from '../shared';
  import { localizeMatchQuality, localizeOwnedText, localizeState, t, td, type MessageKey } from '../../lib/i18n';

  $: snapshot = $streamState.snapshot;
  $: hub = $streamState.hub;
  $: installPlan = $streamState.installPlan;
  $: installStatus = $streamState.installStatus;
  $: busy = $shellState.busy;
  $: locale = $shellState.locale;
  $: filter = $driversUi.filter;
  $: search = $driversUi.search;
  $: selected = $driversUi.selected;
  $: expanded = $driversUi.expanded;

  const filterKeys: Record<string, MessageKey> = {
    All:'drivers.filter.All', Updates:'drivers.filter.Updates', Problems:'drivers.filter.Problems', Missing:'drivers.filter.Missing', Display:'drivers.filter.Display'
  };
  const scanStepKeys = [
    ['InventoryScanning','drivers.step.inventory','drivers.step.inventoryHint'],
    ['UpdateSearching','drivers.step.update','drivers.step.updateHint'],
    ['Matching','drivers.step.match','drivers.step.matchHint'],
    ['Ready','drivers.step.ready','drivers.step.readyHint'],
  ] as const;

  function selectedMinBytes(): number { return selectedUpdates().reduce((sum, candidate) => sum + candidate.minDownloadBytes, 0); }
  function selectedMaxBytes(): number { return selectedUpdates().reduce((sum, candidate) => sum + candidate.maxDownloadBytes, 0); }
  function owned(value: string) { return localizeOwnedText(value, locale); }
  function heroTitle(): string {
    if (hub.state === 'Idle') return t('drivers.hero.idle',locale);
    if (hub.state === 'Ready') return t('drivers.hero.ready',locale);
    if (hub.state === 'Failed') return t('drivers.hero.failed',locale);
    if (hub.state === 'InventoryScanning') return t('drivers.hero.inventory',locale);
    if (hub.state === 'UpdateSearching') return t('drivers.hero.update',locale);
    return t('drivers.hero.matching',locale);
  }
</script>

<header class="drivers-header">
  <div><p class="eyebrow">{t('drivers.eyebrow',locale)}</p><h1>{t('drivers.title',locale)}</h1><p class="sub">{t('drivers.subtitle',locale)}</p></div>
  <div class="header-actions">
    <div class="service-pill"><span class:online={snapshot.connected}></span>{snapshot.connected ? t('common.engineOnline',locale,{version:snapshot.serviceVersion}) : t('common.engineOffline',locale)}</div>
    {#if hub.state === 'Ready' && hub.summary.recommendedUpdateCount > 0}<Pressable className="primary" onclick={() => { selectAllRecommended(); reviewDriverInstall(); }} disabled={busy || !snapshot.connected}>{t('drivers.updateRecommended',locale,{count:hub.summary.recommendedUpdateCount})}</Pressable>{/if}
    <Pressable className="scan-button" onclick={startScan} disabled={busy || scanStates.includes(hub.state) || !snapshot.connected}><span>↻</span>{hub.state === 'Idle' ? t('drivers.scan',locale) : scanStates.includes(hub.state) ? t('drivers.scanning',locale) : t('drivers.scanAgain',locale)}</Pressable>
  </div>
</header>

<section class="driver-hero">
  <div class="driver-hero-copy">
    <div class:scanning={scanStates.includes(hub.state)} class:ready={hub.state === 'Ready'} class="scan-orb"><span>{hub.state === 'Ready' ? '✓' : hub.state === 'Failed' ? '!' : '◫'}</span></div>
    <div><p class="eyebrow">{t('drivers.liveDiscovery',locale)}</p><h2>{heroTitle()}</h2>
      {#if hub.state === 'Ready'}<p>{t('drivers.inventorySummary',locale,{epoch:hub.inventoryEpoch,count:hub.summary.deviceCount})}</p>
      {:else if hub.state === 'Idle'}<p>{t('drivers.catalogPolicy',locale)}</p>
      {:else if hub.errorMessage}{@const err=owned(hub.errorMessage)}{#if err.localized}<p>{err.text}</p>{:else}<p><TechnicalText value={hub.errorMessage}/></p>{/if}
      {:else}<p>{t('drivers.scanNative',locale)}</p>{/if}
    </div>
  </div>
  {#if scanStates.includes(hub.state)}<div class="indeterminate"><span></span></div>{/if}
  <div class="scan-steps">
    {#each scanStepKeys as step, i}
      <div class:done={hub.state === 'Ready' || scanStateIndex(hub.state) > i} class:current={hub.state === step[0]}><span>{i + 1}</span><div><strong>{td(step[1],locale)}</strong><small>{td(step[2],locale)}</small></div></div>
    {/each}
  </div>
</section>

<section class="driver-metrics">
  <article><span>{t('drivers.metric.devices',locale)}</span><strong>{hub.summary.deviceCount}</strong><small>{t('drivers.metric.devicesHint',locale)}</small></article>
  <article class:attention={hub.summary.recommendedUpdateCount > 0}><span>{t('drivers.metric.recommended',locale)}</span><strong>{hub.summary.recommendedUpdateCount}</strong><small>{t('drivers.metric.recommendedHint',locale)}</small></article>
  <article class:warning={hub.summary.missingDriverCount > 0}><span>{t('drivers.metric.missing',locale)}</span><strong>{hub.summary.missingDriverCount}</strong><small>{t('drivers.metric.missingHint',locale)}</small></article>
  <article><span>{t('drivers.metric.vendorManaged',locale)}</span><strong>{hub.summary.managementAuthorityCount}</strong><small>{t('drivers.metric.vendorManagedHint',locale)}</small></article>
</section>

{#if hub.warnings.length}
  <section class="warning-strip"><span>◇</span><div><strong>{t('drivers.discoveryNote',locale)}</strong>
    {#each hub.warnings as warning}{@const msg=owned(warning)}{#if msg.localized}<p>{msg.text}</p>{:else}<p><TechnicalText value={warning}/></p>{/if}{/each}
  </div></section>
{/if}

{#if hub.state === 'Ready'}
  <section class:warning={hub.authorityCoverage !== 'CompleteForRequiredAuthorities'} class="authority-coverage"><div><strong>{t('drivers.coverage.title',locale)}</strong><p>{t(`drivers.coverage.${hub.authorityCoverage}` as MessageKey,locale)}</p></div><div class="selection-tools"><button use:fluidPress={{ pressedScale:0.985 }} onclick={selectAllRecommended}>{t('drivers.selectAllRecommended',locale)}</button><button use:fluidPress={{ pressedScale:0.985 }} onclick={clearDriverSelection}>{t('drivers.clearAll',locale)}</button></div></section>
{/if}

{#if selectedCandidates().length > 0 && !installActive() && !(installPlan && installPlan.state === 'AwaitingAuthorization')}
  <section class="selection-tray"><div><span class="selection-count">{selectedCandidates().length}</span><div><strong>{t('drivers.selectionTitle',locale)}</strong><p>{t('drivers.selectionCopy',locale,{updates:selectedUpdates().length,range:formatRange(selectedMinBytes(),selectedMaxBytes(),locale)})}</p></div></div><button use:fluidPress={{ pressedScale: 0.985 }} class="install-button" onclick={reviewDriverInstall} disabled={busy || hub.state !== 'Ready'}>{t('drivers.reviewInstall',locale)}</button></section>
{/if}

{#if installPlan && !installStatus && installPlan.state === 'AwaitingAuthorization'}
  <section class="pending-install-card">
    <div><p class="eyebrow">{t('drivers.immutablePlan',locale)}</p><h3>{t('drivers.planWaiting',locale)}</h3><p>{t('drivers.planSummary',locale,{count:installPlan.actionCount,epoch:installPlan.inventoryEpoch,digest:shortDigest(installPlan.digest)})}</p></div>
    <div class="pending-install-actions">{#if installPlan.consentReadyUntilUnixMs > Date.now()}<span class="authorized-chip">{t('drivers.uacApproved',locale)}</span><button use:fluidPress={{ pressedScale: 0.985 }} class="install-button" onclick={startDriverInstall} disabled={busy}>{t('overview.startInstall',locale)}</button>{:else}<button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={authorizeDriverPlan} disabled={busy}>{t('drivers.approveUac',locale)}</button>{/if}</div>
  </section>
{/if}

{#if installStatus}
  {@const statusDetail=owned(installStatus.detail)}
  <section class:terminal={installTerminal()} class="install-progress-card">
    <div class="install-progress-head"><div><p class="eyebrow">{t('drivers.execution',locale)}</p><h3>{localizeState(installStatus.stage,locale)}</h3>{#if statusDetail.localized}<p>{statusDetail.text}</p>{:else}<p><TechnicalText value={installStatus.detail}/></p>{/if}</div><span class:bad={stageTone(installStatus.stage)==='bad'} class:warn={stageTone(installStatus.stage)==='warn'} class="execution-state">{localizeState(installStatus.planState,locale)}</span></div>
    {#if installStatus.progressKnown}
      <ProgressBar value={installStatus.overallPercent} label={t('drivers.progressLabel',locale)} />
      <div class="progress-caption"><span>{t('drivers.progressKnown',locale,{percent:installStatus.overallPercent})}{installStatus.bytesTotal ? ` · ${formatBytes(installStatus.bytesDownloaded,locale)} / ${formatBytes(installStatus.bytesTotal,locale)}` : ''}</span><span>{installStatus.mutationStarted ? t('drivers.mutationStarted',locale) : t('drivers.noMutation',locale)}</span></div>
    {:else if installActive()}<ProgressBar value={0} known={false} label={t('drivers.installingLabel',locale)} />{/if}
    <div class="protection-grid">
      <div class:ready={installStatus.restorePointVerified}><span>{t('drivers.restorePoint',locale)}</span><strong>{installStatus.restorePointVerified ? t('drivers.verifiedSequence',locale,{sequence:installStatus.restorePointSequence}) : t('common.pending',locale)}</strong></div>
      <div class:ready={!!installStatus.backupRoot}><span>{t('drivers.driverBackup',locale)}</span><strong>{installStatus.backupRoot ? t('drivers.evidenceCaptured',locale) : t('common.pending',locale)}</strong></div>
      <div class:ready={installStatus.mutationStarted}><span>{t('drivers.wuaGate',locale)}</span><strong>{installStatus.mutationStarted ? t('common.crossed',locale) : t('common.protected',locale)}</strong></div>
      <div class:warn={installStatus.rebootRequired}><span>{t('drivers.restart',locale)}</span><strong>{installStatus.rebootRequired ? t('common.required',locale) : t('common.notRequested',locale)}</strong></div>
    </div>
    {#if installStatus.rebootRequired}<div class="reboot-banner"><strong>{t('drivers.restartRequired',locale)}</strong><p>{t('drivers.restartCopy',locale)}</p></div>{/if}
    {#if installStatus.failureMessage}{@const failure=owned(installStatus.failureMessage)}<div class="install-failure"><strong>{t('drivers.executionNote',locale)}</strong>{#if failure.localized}<p>{failure.text}</p>{:else}<p><TechnicalText value={installStatus.failureMessage}/></p>{/if}</div>{/if}
    <div class="install-items">
      {#each installStatus.items as item (item.candidateId)}
        <div class="install-item"><div><strong><TechnicalText value={item.title}/></strong><p><TechnicalText value={item.instanceId}/></p></div><div><span>{localizeState(item.stage,locale)}</span><small><TechnicalText value={`${item.beforeVersion || '—'} → ${item.afterVersion || '—'}`}/></small></div><div>{#if item.progressKnown}<strong>{item.progressPercent}%</strong>{:else}<strong>—</strong>{/if}<small>{#if item.verified}{t('drivers.pnpVerified',locale)}{:else if item.resultCode}<TechnicalText value={item.resultCode}/>{:else}{t('common.waiting',locale)}{/if}</small></div></div>
      {/each}
    </div>
  </section>
{/if}

<section class="driver-panel">
  <div class="driver-toolbar"><div class="filters">{#each driverFilters as name}<button use:fluidPress={{ pressedScale: 0.985 }} class:active={filter === name} aria-pressed={filter === name} onclick={() => setDriverFilter(name)}>{td(filterKeys[name],locale)}</button>{/each}</div><label class="search-box"><span>⌕</span><input value={search} oninput={(event) => setDriverSearch((event.currentTarget as HTMLInputElement).value)} placeholder={t('drivers.searchPlaceholder',locale)} /></label></div>

  {#if hub.state === 'Idle'}
    <div class="driver-empty"><div>◫</div><h3>{t('drivers.emptyTitle',locale)}</h3><p>{t('drivers.emptyCopy',locale)}</p><button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={startScan} disabled={!snapshot.connected}>{t('drivers.scan',locale)}</button></div>
  {:else if scanStates.includes(hub.state) && !hub.devices.length}
    <div class="driver-empty scanning-empty"><div>↻</div><h3>{t('drivers.discoveryProgressTitle',locale)}</h3><p>{t('drivers.discoveryProgressCopy',locale)}</p></div>
  {:else if hub.state === 'Failed'}
    {@const fail=owned(hub.errorMessage)}
    <div class="driver-empty"><div>!</div><h3>{t('drivers.failedTitle',locale)}</h3>{#if fail.localized}<p>{fail.text}</p>{:else}<p><TechnicalText value={hub.errorMessage}/></p>{/if}<button use:fluidPress={{ pressedScale: 0.985 }} class="secondary" onclick={startScan}>{t('common.tryAgain',locale)}</button></div>
  {:else}
    <div class="device-table-head"><span>{t('common.device',locale)}</span><span>{t('drivers.column.current',locale)}</span><span>{t('drivers.column.offer',locale)}</span><span>{t('common.status',locale)}</span><span></span></div>
    <div class="device-list">
      {#each filteredDevices() as device (device.instanceId)}
        <article class:problem={device.hasProblem} class:gpu={device.displayManaged} class="device-card">
          <div class="device-main">
            <div class="device-identity"><div class="device-icon">{device.displayManaged ? '▰' : device.className === 'Net' ? '⌁' : device.className === 'MEDIA' ? '◉' : '◇'}</div><div><strong><TechnicalText value={device.displayName}/></strong><p>{#if device.manufacturer}<TechnicalText value={device.manufacturer}/>{:else}{t('drivers.manufacturerUnknown',locale)}{/if} · {#if device.className}<TechnicalText value={device.className}/>{:else}{t('drivers.unclassified',locale)}{/if}</p></div></div>
            <div class="driver-current"><small>{device.driver?.provider || t('drivers.noDriverMetadata',locale)}</small><strong><TechnicalText value={device.driver?.version || '—'}/></strong><span><TechnicalText value={device.driver?.date || device.driver?.infPath || ''}/></span></div>
            <div class="driver-target">{#if device.candidates.length}<small>{device.candidates[0].provider || t('drivers.step.update',locale)}</small><strong>{#if device.candidates[0].targetVersion}<TechnicalText value={device.candidates[0].targetVersion}/>{:else}{targetLabel(device.candidates[0],locale)}{/if}</strong><span>{targetEvidence(device.candidates[0],locale)} · {formatRange(device.candidates[0].minDownloadBytes,device.candidates[0].maxDownloadBytes,locale)}</span>{:else}<small>{t('drivers.step.update',locale)}</small><strong>—</strong><span>{t('drivers.noOffer',locale)}</span>{/if}</div>
            <div class="device-status"><span class:bad={device.hasProblem} class:update={device.candidates.some((c) => c.selectable)} class:vendor={device.displayManaged || device.candidates.some((candidate) => candidate.firmwareManaged)}>{stateLabel(device,locale)}</span></div>
            <button use:fluidPress={{ pressedScale: 0.985 }} class="expand-button" aria-label={t('drivers.toggleDetails',locale)} onclick={() => toggleExpanded(device.instanceId)}>{expanded[device.instanceId] ? '−' : '+'}</button>
          </div>

          {#if device.candidates.length}
            <div class="candidate-list">{#each device.candidates as candidate (candidate.candidateId)}<label class:locked={!candidate.selectable} class:recommended={candidate.recommendationState === 'Recommended'} class="candidate-row"><input type="checkbox" checked={!!selected[candidate.candidateId]} disabled={!candidate.selectable} onchange={(event) => toggleCandidate(candidate.candidateId,(event.currentTarget as HTMLInputElement).checked)}/><span class="candidate-check"></span><div><div class="candidate-title-line"><strong><TechnicalText value={candidate.title}/></strong>{#if candidate.recommendationState === 'Recommended'}<span class="recommend-chip">{t('drivers.recommended',locale)}</span>{:else if candidate.recommendationState === 'Alternative'}<span class="alternative-chip">{t('drivers.alternative',locale)}</span>{/if}</div><p>{localizeMatchQuality(candidate.matchQuality,locale)} · <TechnicalText value={candidate.matchedHardwareId}/></p><p class="authority-line"><span class="source-chip">{candidate.authorityName || candidate.provider || t('drivers.officialSource',locale)}</span> · {t(`drivers.installMode.${candidate.installationMode}` as MessageKey,locale)}</p></div><div class="candidate-meta"><strong>{#if candidate.targetVersion}<TechnicalText value={candidate.targetVersion}/>{:else}{targetLabel(candidate,locale)}{/if}</strong><small>{targetEvidence(candidate,locale)} · {formatRange(candidate.minDownloadBytes,candidate.maxDownloadBytes,locale)}</small><small>{#if candidate.recommendationReasons.length}<TechnicalText value={candidate.recommendationReasons[0]}/>{/if}</small></div>{#if candidate.firmwareManaged}<span class="vendor-chip">{t('drivers.chip.firmwareReview',locale)}</span>{:else if candidate.vendorManaged}<span class="vendor-chip">{t('drivers.chip.officialUtility',locale)}</span>{:else}<span class="windows-chip">{candidate.authorityName || t('drivers.chip.windowsOffer',locale)}</span>{/if}</label>{/each}</div>
          {/if}

          {#if device.managementAuthorities.length}
            <div class="management-authorities">
              {#each device.managementAuthorities as authority (authority.providerId)}
                <div class="gpu-policy"><div class="gpu-logo">{authority.displayName.slice(0,1)}</div><div><strong>{t('drivers.officialManagement',locale)} · <TechnicalText value={authority.displayName}/></strong><p>{t(`drivers.management.${authority.availability}` as MessageKey,locale)} · {t(`drivers.updateAvailability.${authority.updateAvailability}` as MessageKey,locale)}</p><small><TechnicalText value={authority.providerId}/></small></div>{#if device.gpu && authority.providerId === device.gpu.providerId}<button use:fluidPress={{ pressedScale: 0.985 }} class="vendor-link" onclick={() => openGpuSupport(device.gpu?.vendor ?? '')}>{device.gpu.appInstalled ? t('drivers.openApp',locale,{app:device.gpu.appName}) : t('drivers.manualOfficialCheck',locale)} ↗</button>{/if}</div>
              {/each}
            </div>
          {/if}

          {#if expanded[device.instanceId]}
            {#if device.candidates.length}<div class="candidate-preferences" aria-label={t('drivers.preference.title',locale)}>{#each device.candidates as candidate (candidate.candidateId)}<div><TechnicalText value={candidate.title}/><span><button use:fluidPress={{ pressedScale:0.985 }} onclick={() => setDriverCandidatePolicy(candidate.candidateId,'IgnoreExactVersion')}>{t('drivers.preference.ignoreVersion',locale)}</button><button use:fluidPress={{ pressedScale:0.985 }} onclick={() => setDriverCandidatePolicy(candidate.candidateId,'RemindLater')}>{t('drivers.preference.remindLater',locale)}</button>{#if candidate.recommendationState === 'Optional'}<button use:fluidPress={{ pressedScale:0.985 }} onclick={() => setDriverCandidatePolicy(candidate.candidateId,'IgnoreOptional')}>{t('drivers.preference.ignoreOptional',locale)}</button>{/if}</span></div>{/each}</div>{/if}
            <div class="device-details"><div><span>{t('drivers.updateStatus',locale)}</span><code>{stateLabel(device,locale)}</code></div><div><span>{t('drivers.authorityCoverage',locale)}</span><code>{t(`drivers.coverage.${device.authorityCoverage}` as MessageKey,locale)}</code></div><div><span>{t('drivers.instanceId',locale)}</span><code>{device.instanceId}</code></div><div><span>{t('drivers.classGuid',locale)}</span><code>{device.classGuid || '—'}</code></div><div><span>{t('common.location',locale)}</span><code>{device.location || t('common.notReported',locale)}</code></div><div><span>{t('drivers.problemCode',locale)}</span><code>{device.problemCode}</code></div><div class="full"><span>{t('drivers.hardwareIds',locale)}</span>{#if device.hardwareIds.length}{#each device.hardwareIds as id}<code>{id}</code>{/each}{:else}<code>{t('common.notReported',locale)}</code>{/if}</div><div class="full"><span>{t('drivers.compatibleIds',locale)}</span>{#if device.compatibleIds.length}{#each device.compatibleIds as id}<code>{id}</code>{/each}{:else}<code>{t('common.notReported',locale)}</code>{/if}</div><div class="full"><span>{t('drivers.authority.required',locale)}</span>{#if device.requiredAuthorities.length}{#each device.requiredAuthorities as id}<code>{id}</code>{/each}{:else}<code>—</code>{/if}</div><div class="full"><span>{t('drivers.authority.evaluated',locale)}</span>{#if device.evaluatedAuthorities.length}{#each device.evaluatedAuthorities as id}<code>{id}</code>{/each}{:else}<code>—</code>{/if}</div>{#if device.manualAuthorities.length}<div class="full"><span>{t('drivers.authority.manual',locale)}</span>{#each device.manualAuthorities as id}<code>{id}</code>{/each}</div>{/if}{#if device.unavailableAuthorities.length}<div class="full"><span>{t('drivers.authority.unavailable',locale)}</span>{#each device.unavailableAuthorities as id}<code>{id}</code>{/each}</div>{/if}</div>
          {/if}
        </article>
      {/each}
    </div>
    {#if filteredDevices().length === 0}<div class="filtered-empty">{t('drivers.noFilterMatch',locale)}</div>{/if}
  {/if}
</section>

{#if hub.state === 'Ready' && hub.unmatchedOffers.length > 0}<p class="footnote">{t('drivers.unmatchedOffers',locale,{count:hub.unmatchedOffers.length})}</p>{/if}
