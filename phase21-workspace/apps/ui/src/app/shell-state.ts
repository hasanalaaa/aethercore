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
};

const initialLocale = getInitialLocale();
const busyActivities = new ActivityCounter();
applyLocale(initialLocale);

export const shellState = writable<ShellState>({
  activePage: 'overview',
  busy: false,
  errorMessage: '',
  paletteOpen: false,
  locale: initialLocale,
  liveAnnouncement: '',
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
