<script lang="ts">
  import { TechnicalText } from '../../design/primitives';
  import { formatDateTime, t, td, type Locale } from '../../lib/i18n';
  import type { MeasurementRow } from './measurement-rows';

  export let title: string;
  export let rows: readonly MeasurementRow[];
  export let locale: Locale;
</script>

{#if rows.length}
  <section class="measurement-rows" aria-label={title}>
    <h3>{title}</h3>
    <ul>
      {#each rows as row (row.id)}
        <li>
          <span class="measurement-name"><TechnicalText value={row.name}/></span>
          <strong class="measurement-value">{row.value ?? t('measurement.noValue', locale)}</strong>
          <span class="measurement-state">{td(row.availability, locale)}{#if row.source} · <TechnicalText value={row.source}/>{/if}{#if row.observedUnixMs !== null} · {t('measurement.observed', locale, { time: formatDateTime(row.observedUnixMs, locale) })}{/if}</span>
          {#if row.note}<span class="measurement-note">{row.note}</span>{/if}
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .measurement-rows{margin-block:12px;padding:14px 16px;border:1px solid var(--ac-border-subtle);border-radius:15px;background:var(--ac-material-base)}
  h3{margin:0 0 .5rem;font-size:var(--ac-type-headline);font-weight:590}
  ul{list-style:none;margin:0;padding:0;display:grid;gap:8px}
  li{display:grid;grid-template-columns:minmax(0,1fr) auto;gap:2px 12px;padding-block:6px;border-block-start:1px solid var(--ac-border-subtle)}
  li:first-child{border-block-start:0}
  .measurement-name{min-inline-size:0;overflow-wrap:anywhere}
  .measurement-value{text-align:end}
  .measurement-state,.measurement-note{grid-column:1 / -1;color:var(--ac-text-3);font-size:var(--ac-type-body)}
</style>
