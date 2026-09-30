<script lang="ts">
  import { shellState } from '../app/shell-state';
  import { streamState } from '../platform/stream-state';
  import { formatWhen } from './shared';
  import { driverRecoveryMeans, localizeKind, localizeOwnedText, localizeSeverity, t } from '../lib/i18n';
  import { TechnicalText } from '../design/primitives';
  $: recoveryEntries = $streamState.recoveryEntries;
  $: locale = $shellState.locale;
</script>

{#if recoveryEntries.length > 0}
  <section class="recovery-panel">
    <div class="panel-head"><div><h3>{t('recovery.title',locale)}</h3></div></div>
    {#each recoveryEntries as entry (entry.seq)}
      {@const summary = localizeOwnedText(entry.summary,locale)}
      {@const detail = localizeOwnedText(entry.detail,locale)}
      <div class="recovery-row"><span class:warning={entry.severity === 'warning' || entry.severity === 'Amber'}>◇</span><div>
        {#if summary.localized}<strong>{summary.text}</strong>{:else}<strong><TechnicalText value={entry.summary}/></strong>{/if}
        {#if detail.localized}<p>{detail.text}</p>{:else}<p><TechnicalText value={entry.detail}/></p>{/if}
        <!-- P84-05: what exists to recover with, and the manual way back. No rollback button:
             AetherCore does not roll a driver back (D17). -->
        {#each driverRecoveryMeans(entry,locale) as line, index (index)}<p>{line}</p>{#if index === 1 && entry.backupRoot}<p><TechnicalText value={entry.backupRoot}/></p>{/if}{/each}
        <small>{formatWhen(entry.createdUnixMs,locale)} · {localizeKind(entry.kind,locale)} · {localizeSeverity(entry.severity,locale)}</small>
      </div></div>
    {/each}
  </section>
{/if}
