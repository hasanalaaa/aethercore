<script lang="ts">
  import AppIcon from './AppIcon.svelte';
  import { fluidPress } from '../design/motion';
  import { NAVIGATION, NAVIGATION_GROUPS, type PageId } from '../lib/navigation';
  import { t, td, type Locale } from '../lib/i18n';

  export let activePage: PageId;
  export let connected = false;
  export let serviceVersion = '—';
  export let locale: Locale = 'en';
  export let paletteOpen = false;
  export let theme: 'dark' | 'light' = 'dark';
  export let onNavigate: (page: PageId) => void;
  export let onOpenPalette: () => void;
  export let onToggleLocale: () => void;
  export let onToggleTheme: () => void;

  function navKey(event: KeyboardEvent) {
    const buttons = Array.from((event.currentTarget as HTMLElement).closest('nav')?.querySelectorAll<HTMLButtonElement>('button[data-nav-item]') ?? []);
    if (!buttons.length) return;
    const index = buttons.indexOf(event.currentTarget as HTMLButtonElement);
    let target = index < 0 ? 0 : index;
    if (event.key === 'ArrowDown') target = (index + 1) % buttons.length;
    else if (event.key === 'ArrowUp') target = (index - 1 + buttons.length) % buttons.length;
    else if (event.key === 'Home') target = 0;
    else if (event.key === 'End') target = buttons.length - 1;
    else return;
    event.preventDefault();
    buttons[target]?.focus();
  }
</script>

<aside class="app-sidebar" aria-label={t('app.primaryNavigation', locale)}>
  <div class="app-brand">
    <div class="brand-mark" aria-hidden="true">
      <svg viewBox="0 0 64 64" focusable="false" aria-hidden="true">
        <path d="M12 51 L28 12 L28 51" />
        <path d="M28 12 L51 12" />
        <path d="M28 31.5 L43 31.5" />
        <path d="M28 51 L51 51" />
      </svg>
    </div>
    <div class="brand-copy"><strong>AetherCore</strong><small>{t('app.subtitle', locale)}</small></div>
  </div>

  <button use:fluidPress={{ pressedScale: 0.985 }} class="command-trigger" type="button" onclick={onOpenPalette} aria-label={t('app.command', locale)} aria-keyshortcuts="Control+K" aria-haspopup="dialog" aria-expanded={paletteOpen}>
    <AppIcon name="search" size={16}/><span>{t('app.command', locale)}</span><kbd>Ctrl K</kbd>
  </button>

  <nav aria-label={t('app.primaryNavigation', locale)}>
    {#each NAVIGATION_GROUPS as group}
      <div class="nav-group">
        <h2>{td(group.labelKey, locale)}</h2>
        {#each NAVIGATION.filter((item) => item.group === group.id) as item (item.id)}
          <button
            type="button"
            use:fluidPress={{ pressedScale: 0.992 }}
            data-nav-item
            data-page={item.id}
            class="app-nav-item"
            class:active={activePage === item.id}
            aria-current={activePage === item.id ? 'page' : undefined}
            aria-keyshortcuts={item.shortcut.replace('Ctrl', 'Control')}
            tabindex={activePage === item.id ? 0 : -1}
            aria-label={`${td(item.labelKey, locale)} — ${td(item.descriptionKey, locale)}`}
            title={`${td(item.descriptionKey, locale)} · ${item.shortcut}`}
            onclick={() => onNavigate(item.id)}
            onkeydown={navKey}
          >
            <span class="nav-icon"><AppIcon name={item.icon} size={16}/></span>
            <span class="nav-copy"><strong>{td(item.labelKey, locale)}</strong><small>{td(item.descriptionKey, locale)}</small></span>
            {#if item.availability === 'online' && !connected}<span class="nav-availability" title={t('palette.unavailable', locale)} aria-label={t('palette.unavailable', locale)}>·</span>{/if}
          </button>
        {/each}
      </div>
    {/each}
  </nav>

  <div class="policy-card" role="note">
    <span class="policy-mark" aria-hidden="true"><svg viewBox="0 0 24 24" width="13" height="13" fill="none"><circle cx="12" cy="12" r="8.25"/><path d="M6.2 6.2l11.6 11.6"/></svg></span>
    <div><strong>{t('app.policyTitle', locale)}</strong><small>{t('app.policyCopy', locale)}</small></div>
  </div>

  <div class="sidebar-controls">
    <button type="button" use:fluidPress={{ pressedScale: 0.985 }} class="locale-button" onclick={onToggleLocale} aria-label={t('app.language', locale)}>
      <AppIcon name="language" size={16}/><span lang={locale === 'en' ? 'ar' : 'en'} dir={locale === 'en' ? 'rtl' : 'ltr'}>{locale === 'en' ? t('locale.switchToArabic', locale) : t('locale.switchToEnglish', locale)}</span>
    </button>
    <button type="button" use:fluidPress={{ pressedScale: 0.985 }} class="locale-button theme-button" onclick={onToggleTheme} aria-label={t('app.theme', locale)}>
      <AppIcon name="theme" size={16}/><span>{theme === 'dark' ? t('app.themeLight', locale) : t('app.themeDark', locale)}</span>
    </button>
  </div>

  <div class="app-service" aria-live="polite">
    <div class:online={connected} class="status-dot"></div>
    <div class="service-copy"><small>{t('service.name', locale)}</small><strong>{connected ? t('service.connected', locale) : t('service.offline', locale)}</strong></div>
    {#if connected}<span class="service-version" title={`${t('service.engine', locale)} ${serviceVersion}`}>{serviceVersion}</span>{/if}
  </div>
</aside>
