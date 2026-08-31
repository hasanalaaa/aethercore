import { get, writable } from 'svelte/store';
import { NAVIGATION, type PageId } from '../lib/navigation';
import { applyLocale, getInitialLocale, t, td, type Locale } from '../lib/i18n';
import { ActivityCounter } from './activity-counter';

export type ShellState = {
  activePage: PageId;
  busy: boolean;
  errorMessage: string;
  paletteOpen: boolean;
  locale: Locale;
  liveAnnouncement: string;
  theme: 'dark' | 'light';
};

const initialLocale = getInitialLocale();
function getInitialTheme(): 'dark' | 'light' {
  try {
    const stored = localStorage.getItem('aethercore.theme');
    if (stored === 'light' || stored === 'dark') return stored;
  } catch { /* hardened/no-storage context */ }
  return window.matchMedia?.('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}
function applyTheme(theme: 'dark' | 'light'): void {
  document.documentElement.dataset.theme = theme;
  document.documentElement.dataset.systemTheme = theme;
  document.documentElement.style.colorScheme = theme;
  try { localStorage.setItem('aethercore.theme', theme); } catch { /* non-fatal */ }
}
const busyActivities = new ActivityCounter();
applyLocale(initialLocale);
const initialTheme = getInitialTheme();
applyTheme(initialTheme);

export const shellState = writable<ShellState>({
  activePage: 'overview',
  busy: false,
  errorMessage: '',
  paletteOpen: false,
  locale: initialLocale,
  liveAnnouncement: '',
  theme: initialTheme,
});

export function setBusy(busy: boolean): void {
  shellState.update((state) => ({ ...state, busy }));
}

export function setError(error: unknown): void {
  shellState.update((state) => ({ ...state, errorMessage: error ? String(error) : '' }));
}

export function clearError(): void { setError(''); }

export function setPage(activePage: PageId): void {
  shellState.update((state) => {
    const navigation = NAVIGATION.find((item) => item.id === activePage);
    const pageLabel = navigation ? td(navigation.labelKey, state.locale) : activePage;
    return {
      ...state,
      activePage,
      paletteOpen: false,
      liveAnnouncement: t('announce.page', state.locale, { page: pageLabel }),
    };
  });
}

export function setPaletteOpen(paletteOpen: boolean): void {
  shellState.update((state) => ({ ...state, paletteOpen }));
}

export function announce(liveAnnouncement: string): void {
  shellState.update((state) => ({ ...state, liveAnnouncement }));
}

export function toggleLocale(): void {
  shellState.update((state) => {
    const locale: Locale = state.locale === 'en' ? 'ar' : 'en';
    applyLocale(locale);
    return {
      ...state,
      locale,
      liveAnnouncement: locale === 'ar' ? t('announce.languageArabic', locale) : t('app.languageChanged', locale),
    };
  });
}

export function toggleTheme(): void {
  shellState.update((state) => {
    const theme = state.theme === 'dark' ? 'light' : 'dark';
    applyTheme(theme);
    return { ...state, theme, liveAnnouncement: t(theme === 'light' ? 'announce.themeLight' : 'announce.themeDark', state.locale) };
  });
}

export async function runBusy<T>(operation: () => Promise<T>): Promise<T | undefined> {
  const release = busyActivities.enter();
  setBusy(true);
  clearError();
  try {
    return await operation();
  } catch (error) {
    setError(error);
    return undefined;
  } finally {
    release();
    setBusy(busyActivities.active);
  }
}

export function currentShellState(): ShellState { return get(shellState); }
