<script lang="ts">
  import { fluidPress } from '../../design/motion';
  import { setAssistantOpen } from '../../app/shell-state';
  import { t, type Locale } from '../../lib/i18n';
  import { assistantState, loadAssistantPack, suggestedAssistantQuestion } from './controller';
  import { knownContext } from './context';
  export let evidenceId: string;
  export let surface: string;
  export let locale: Locale;
  $: available = knownContext($assistantState.pack, evidenceId, surface);
  let requested = '';
  $: if (evidenceId && requested !== evidenceId) { requested = evidenceId; void loadAssistantPack(); }
  function open(): void {
    if (!available) return;
    suggestedAssistantQuestion.set(t('assistant.contextQuestion', locale));
    setAssistantOpen(true);
  }
</script>
{#if available}<button use:fluidPress={{ pressedScale: 0.985 }} type="button" class="secondary" onclick={open}>{t('assistant.askFinding', locale)}</button>{/if}
