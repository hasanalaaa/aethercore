<script lang="ts">
  /**
   * Phase 27 — About/Diagnostics panel: the live PlatformCapabilities matrix with typed
   * availability chips plus the honest performance engine source (native/synthetic).
   * Every state and reason key resolves through the message catalogs (EN/AR parity).
   */
  import { onMount } from 'svelte';
  import { fluidPress } from '../design/motion';
  import { shellState } from '../app/shell-state';
  import { EmptyState } from '../design/signature';
  import { serviceInvoke } from '../platform/service-client';
  import { TechnicalText } from '../design/primitives';
  import { t, td, hasMessageKey } from '../lib/i18n';

  $: locale = $shellState.locale;

  type CapabilityAvailability = { state: string; key: string };
  type CapabilityStatus = { name: string; availability: CapabilityAvailability };
  type PlatformCapabilities = { platform: string; capabilities: CapabilityStatus[] };
  type EngineSource = { source: string; platform: string };

  let matrix = null as PlatformCapabilities | null;
  let engine = null as EngineSource | null;
  let loadError = '';

  function chipClass(state: string): string {
    if (state === 'native') return 'chip native';
    if (state === 'degraded') return 'chip degraded';
    return 'chip unavailable';
  }

  function stateLabel(state: string): string {
    if (state === 'native') return t('about.capabilityState.native', locale);
    if (state === 'degraded') return t('about.capabilityState.degraded', locale);
    return t('about.capabilityState.notAvailable', locale);
  }

  function noteLabel(key: string): string {
    // Reason/note keys resolve through the catalogs when present; raw keys stay visible
    // so a missing translation can never masquerade as a different state.
    return hasMessageKey(key) ? td(key, locale) : key;
  }

  function sourceLabel(source: string): string {
    return source === 'native'
      ? t('about.engineSourceNative', locale)
      : t('about.engineSourceSynthetic', locale);
  }

  async function load(): Promise<void> {
    try {
      const [caps, source] = await Promise.all([
        serviceInvoke<PlatformCapabilities>('get_platform_capabilities'),
        serviceInvoke<EngineSource>('get_engine_source'),
      ]);
      // A transport that answers without a capability list has told us nothing;
      // treat that as not-collected rather than as an empty matrix, so the panel
      // never claims the platform supports zero capabilities.
      matrix = caps?.capabilities?.length ? caps : null;
      engine = source?.source ? source : null;
      loadError = '';
    } catch (error) {
      // The raw string stays in the console for support; the panel says
      // "not collected", because a technical dump is not a user-facing state.
      console.error('[AetherCore] platform capabilities unavailable', error);
      loadError = String(error);
      matrix = null;
      engine = null;
    }
  }

  // Previously `void load;` — a reference, not a call. The panel never requested
  // anything and sat on a loading ellipsis forever, which reads as "still
  // working" rather than "nothing collected". Both reads were wrong.
  onMount(() => { void load(); });
</script>

<section class="about-panel" aria-labelledby="about-title">
  <h2 id="about-title">{t('about.title', locale)}</h2>

  <div class="row" use:fluidPress>
    <span class="label">{t('about.platform', locale)}</span>
    <!-- `—` is the honest reading for a value never collected. -->
    <TechnicalText value={matrix?.platform ?? '—'} />
  </div>

  <div class="row" use:fluidPress>
    <span class="label">{t('about.engineSource', locale)}</span>
    <span class="source">{engine ? sourceLabel(engine.source) : '—'}</span>
  </div>

  <h3>{t('about.capabilitiesTitle', locale)}</h3>
  {#if matrix}
    <ul class="capabilities">
      {#each matrix.capabilities as capability (capability.name)}
        <li class="capability">
          <TechnicalText value={capability.name} />
          <span class={chipClass(capability.availability.state)}>{stateLabel(capability.availability.state)}</span>
          {#if capability.availability.key}
            <small class="note">{noteLabel(capability.availability.key)}</small>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <EmptyState
      title={t('common.notCollected', locale)}
      body={t('about.capabilitiesEmpty', locale)}
    />
  {/if}
</section>

<style>
  .about-panel { display: grid; gap: 12px; padding: 20px 0; }
  h2 { font-size:var(--ac-type-title); font-weight: 600; margin: 0; color: var(--ac-text-1); }
  h3 { font-size:var(--ac-type-body); font-weight: 600; margin: 8px 0 0; color: var(--ac-text-2); }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 6px 0; border-bottom: 1px solid var(--ac-border-subtle); }
  .label { color: var(--ac-text-2); font-size:var(--ac-type-body); }
  .source { color: var(--ac-text-1); font-size:var(--ac-type-body); }
  .capabilities { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  .capability { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; font-size:var(--ac-type-body); }
  .chip { border-radius: 999px; padding: 2px 10px; font-size:var(--ac-type-technical); font-weight: 600; letter-spacing: 0.02em; }
  /* Roles, not literals. These three fell back to hardcoded green/orange/grey
     because --ac-positive-bg and --ac-neutral-bg were never defined. */
  .chip.native { background: var(--role-healthy-wash); color: var(--role-healthy); }
  .chip.degraded { background: var(--role-attention-wash); color: var(--role-attention); }
  .chip.unavailable { background: var(--ac-glass-2); color: var(--ac-text-3); }
  .note { width: 100%; color: var(--ac-text-3); font-size: var(--ac-type-technical); direction: ltr; }
</style>
