<script lang="ts">
  /**
   * Settings — the destination four Overview sections needed and the app did
   * not have (§51.3, closing `DBT-P50-003`).
   *
   * Nothing here is new. `SystemCarePanel` (update channel + support bundle) and
   * `AboutPanel` (build identity + platform capability matrix) are the same two
   * components, unmodified, moved off a screen that reports what the machine is
   * doing onto one that holds what the user configures and what the build is.
   * The test for each was the brief's: configuration, not status.
   */
  import { shellState } from '../../app/shell-state';
  import { streamState } from '../../platform/stream-state';
  import { t } from '../../lib/i18n';
  import SystemCarePanel from '../system-care/SystemCarePanel.svelte';
  import AboutPanel from '../../components/AboutPanel.svelte';

  $: locale = $shellState.locale;
  $: snapshot = $streamState.snapshot;
</script>

<header>
  <!-- Title only. "Nothing on this screen reports the state of your machine"
       described the screen's job; the two panels on it are a channel choice and
       a build identity, which show it. -->
  <div><h1>{t('settings.title',locale)}</h1></div>
  <div class="service-pill"><span class:online={snapshot.connected}></span>{snapshot.connected ? t('common.engineOnline',locale,{version:snapshot.serviceVersion}) : t('common.engineOffline',locale)}</div>
</header>

<SystemCarePanel />
<AboutPanel />
