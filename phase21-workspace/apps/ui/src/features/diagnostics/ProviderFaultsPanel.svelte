<script lang="ts">
  import type { ProviderFault } from '../../lib/contracts';
  import type { Locale } from '../../lib/i18n/runtime';
  import { localizeProviderFaultKind, t } from '../../lib/i18n';
  import { TechnicalText } from '../../design/primitives';

  export let faults: ProviderFault[] = [];
  export let locale: Locale;
</script>

{#if faults.length}
  <section class="provider-fault-panel" aria-labelledby="provider-fault-title">
    <!-- One title, not two: the eyebrow and the heading rendered the SAME key.
         The paragraph under them described in 21 generic words what each row
         below states precisely — which provider, and which fault kind. -->
    <div class="provider-fault-copy">
      <h3 id="provider-fault-title">{t('diagnostics.providerFaults.title', locale)}</h3>
    </div>
    <div class="provider-fault-list">
      {#each faults as fault, index (`${fault.provider}:${fault.operation}:${fault.kindCode}:${index}`)}
        <article class="provider-fault-row">
          <span class="fault-kind">{localizeProviderFaultKind(fault.kindCode, locale)}</span>
          <span class="fault-technical"><TechnicalText value={fault.provider}/> · <TechnicalText value={fault.operation}/></span>
        </article>
      {/each}
    </div>
  </section>
{/if}
