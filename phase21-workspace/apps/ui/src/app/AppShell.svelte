<script lang="ts">
  import { onMount } from 'svelte';
  import NavigationRail from '../components/NavigationRail.svelte';
  import CommandPalette from '../components/CommandPalette.svelte';
  import { FluidPage, TechnicalText } from '../design/primitives';
  import { NAVIGATION, type PageId } from '../lib/navigation';
  import { localizeOwnedText, t } from '../lib/i18n';
  import { initializeWindowUx, type WindowUxCleanup } from '../lib/window-ux';
  import { startKernelSession, type KernelSessionCleanup } from '../platform/kernel-session';
  import { streamState } from '../platform/stream-state';
  import { shellState, setPage, setPaletteOpen, toggleLocale } from './shell-state';
  import PlanDialogs from './PlanDialogs.svelte';
  import RecoveryPanel from '../features/RecoveryPanel.svelte';
  import OverviewPage from '../features/overview/OverviewPage.svelte';
  import DeepScanPage from '../features/intelligence/DeepScanPage.svelte';
  import DriversPage from '../features/drivers/DriversPage.svelte';
  import RepairPage from '../features/repair/RepairPage.svelte';
  import CleanupPage from '../features/cleanup/CleanupPage.svelte';
  import StartupPage from '../features/startup/StartupPage.svelte';
  import PerformancePage from '../features/performance/PerformancePage.svelte';
  import HardwarePage from '../features/diagnostics/HardwarePage.svelte';
  import CrashPage from '../features/diagnostics/CrashPage.svelte';
  import ActivityPage from '../features/activity/ActivityPage.svelte';
  import TimelinePanel from '../features/timeline/TimelinePage.svelte';
  import CarePanel from '../features/care/CarePanel.svelte';

  let windowCleanup: WindowUxCleanup | undefined;
  let kernelCleanup: KernelSessionCleanup | undefined;
  $: localizedError = localizeOwnedText($shellState.errorMessage, $shellState.locale);

  function handleGlobalKeydown(event: KeyboardEvent): void {
    if (event.defaultPrevented) return;
    const target = event.target as HTMLElement | null;
    const editing = target?.matches('input, textarea, select, [contenteditable="true"]') ?? false;
    if ((event.ctrlKey || event.metaKey) && !event.shiftKey && event.key.toLowerCase() === 'k') {
      event.preventDefault(); setPaletteOpen(!$shellState.paletteOpen); return;
    }
    if (!editing && event.ctrlKey && event.shiftKey && /^[1-9]$/.test(event.key)) {
      const item = NAVIGATION[Number(event.key) - 1];
      if (item) { event.preventDefault(); setPage(item.id); }
      return;
    }
    if (event.key === 'Escape' && $shellState.paletteOpen) setPaletteOpen(false);
  }

  onMount(() => {
    let disposed = false;
    document.addEventListener('keydown', handleGlobalKeydown);
    initializeWindowUx().then((cleanup) => disposed ? cleanup() : windowCleanup = cleanup);
    startKernelSession().then((cleanup) => disposed ? cleanup() : kernelCleanup = cleanup);
    return () => { disposed = true; document.removeEventListener('keydown', handleGlobalKeydown); kernelCleanup?.(); windowCleanup?.(); };
  });

  function navigate(page: PageId): void { setPage(page); }
</script>

<a class="skip-link" href="#main-content">{t('app.skip', $shellState.locale)}</a>
<div class="a11y-live" aria-live="polite" aria-atomic="true">{$shellState.liveAnnouncement}</div>

<div class="app-shell" data-page={$shellState.activePage}>
  <NavigationRail activePage={$shellState.activePage} connected={$streamState.snapshot.connected} serviceVersion={$streamState.snapshot.serviceVersion} locale={$shellState.locale} paletteOpen={$shellState.paletteOpen} onNavigate={navigate} onOpenPalette={() => setPaletteOpen(true)} onToggleLocale={toggleLocale}/>
  <main id="main-content" tabindex="-1" aria-busy={$shellState.busy}>
    {#if $shellState.errorMessage}
      <div class="error-banner" role="alert">
        <strong>{t('app.actionNotCompleted', $shellState.locale)}</strong>
        {#if localizedError.localized}<span>{localizedError.text}</span>{:else}<TechnicalText value={$shellState.errorMessage}/>{/if}
      </div>
    {/if}
    {#key $shellState.activePage}
      <FluidPage>
        {#if $shellState.activePage === 'deepScan'}<DeepScanPage />
        {:else if $shellState.activePage === 'drivers'}<DriversPage />
        {:else if $shellState.activePage === 'repair'}<RepairPage />
        {:else if $shellState.activePage === 'cleanup'}<CleanupPage />
        {:else if $shellState.activePage === 'startup'}<StartupPage />
        {:else if $shellState.activePage === 'performance'}<PerformancePage />
        {:else if $shellState.activePage === 'hardware'}<HardwarePage />
        {:else if $shellState.activePage === 'crash'}<CrashPage />
        {:else if $shellState.activePage === 'activity'}
          <ActivityPage />
          <TimelinePanel />
        {:else}<OverviewPage />
          <CarePanel />{/if}
      </FluidPage>
    {/key}
    <RecoveryPanel />
  </main>
</div>

<PlanDialogs />
<CommandPalette open={$shellState.paletteOpen} locale={$shellState.locale} onClose={() => setPaletteOpen(false)} onSelect={navigate} />
