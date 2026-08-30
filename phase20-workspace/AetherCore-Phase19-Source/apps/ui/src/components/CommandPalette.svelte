<script lang="ts">
  import { tick } from 'svelte';
  import AppIcon from './AppIcon.svelte';
  import FluidDialog from '../design/primitives/FluidDialog.svelte';
  import { fluidPress } from '../design/motion';
  import { NAVIGATION, type PageId } from '../lib/navigation';
  import { t, td, type Locale } from '../lib/i18n';

  export let open = false;
  export let locale: Locale = 'en';
  export let onClose: () => void;
  export let onSelect: (page: PageId) => void;

  let query = '';
  let selected = 0;
  let input: HTMLInputElement;
  let lastOpen = false;

  $: items = NAVIGATION.filter((item) => `${td(item.labelKey, locale)} ${td(item.descriptionKey, locale)}`.toLowerCase().includes(query.trim().toLowerCase()));
  $: if (selected >= items.length) selected = Math.max(0, items.length - 1);
  $: if (open && !lastOpen) {
    query = '';
    selected = 0;
    tick().then(() => input?.focus());
  }
  $: lastOpen = open;

  function keydown(event: KeyboardEvent) {
    if (['Escape','ArrowDown','ArrowUp','Enter'].includes(event.key)) event.stopPropagation();
    if (event.key === 'Escape') { event.preventDefault(); onClose(); return; }
    if (event.key === 'ArrowDown') { event.preventDefault(); selected = items.length ? (selected + 1) % items.length : 0; return; }
    if (event.key === 'ArrowUp') { event.preventDefault(); selected = items.length ? (selected - 1 + items.length) % items.length : 0; return; }
    if (event.key === 'Enter' && items[selected]) { event.preventDefault(); onSelect(items[selected].id); }
  }
</script>

<FluidDialog {open} labelledBy="command-title" {onClose} backdropClassName="command-backdrop" className="command-palette" transformOrigin={locale === 'ar' ? '85% 0%' : '15% 0%'}>
  <h2 id="command-title" class="sr-only">{t('palette.title', locale)}</h2>
  <div class="command-search">
    <AppIcon name="search" size={18}/>
    <label class="sr-only" for="command-input">{t('palette.title', locale)}</label>
    <input id="command-input" bind:this={input} bind:value={query} placeholder={t('palette.placeholder', locale)} autocomplete="off" spellcheck="false" role="combobox" aria-autocomplete="list" aria-controls="command-results" aria-expanded="true" aria-activedescendant={items[selected] ? `command-option-${items[selected].id.replaceAll(' ', '-').replaceAll('&', 'and')}` : undefined} onkeydown={keydown} />
    <kbd>Esc</kbd>
  </div>
  <div id="command-results" class="command-results" role="listbox" aria-label={t('palette.title', locale)}>
    {#if items.length}
      {#each items as item, index (item.id)}
        <button id={`command-option-${item.id.replaceAll(' ', '-').replaceAll('&', 'and')}`} type="button" role="option" aria-selected={selected === index} class:selected={selected === index} use:fluidPress={{ pressedScale: 0.99 }} onmouseenter={() => selected = index} onclick={() => onSelect(item.id)}>
          <span class="command-result-icon"><AppIcon name={item.icon} size={18}/></span>
          <span><strong>{td(item.labelKey, locale)}</strong><small>{td(item.descriptionKey, locale)}</small></span>
          <kbd>{item.shortcut.replace('Ctrl+Shift+', 'Ctrl ⇧ ')}</kbd>
        </button>
      {/each}
    {:else}
      <p class="command-empty">{t('palette.empty', locale)}</p>
    {/if}
  </div>
  <footer>{t('palette.hint', locale)}</footer>
</FluidDialog>
