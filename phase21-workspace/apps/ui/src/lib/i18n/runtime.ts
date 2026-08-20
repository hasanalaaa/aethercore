import { enCatalog, type MessageKey } from './catalog.en';
import { arCatalog } from './catalog.ar';
import { enPlurals, type PluralMessageKey } from './plurals.en';
import { arPlurals } from './plurals.ar';

export type Locale = 'en' | 'ar';
export type Direction = 'ltr' | 'rtl';
type Primitive = string | number;
type ExtractVars<S extends string> = S extends `${string}{${infer V}}${infer R}` ? V | ExtractVars<R> : never;
type VarsFor<K extends MessageKey> = Record<ExtractVars<(typeof enCatalog)[K]>, Primitive>;
type Args<K extends MessageKey> = ExtractVars<(typeof enCatalog)[K]> extends never ? [vars?: Record<string, never>] : [vars: VarsFor<K>];

const catalogs = { en: enCatalog, ar: arCatalog } as const;

export function getInitialLocale(): Locale {
  try {
    const stored = localStorage.getItem('aethercore.locale');
    if (stored === 'ar' || stored === 'en') return stored;
  } catch { /* hardened/no-storage context */ }
  return navigator.language.toLowerCase().startsWith('ar') ? 'ar' : 'en';
}

export function directionFor(locale: Locale): Direction { return locale === 'ar' ? 'rtl' : 'ltr'; }

export function applyLocale(locale: Locale): void {
  document.documentElement.lang = locale;
  document.documentElement.dir = directionFor(locale);
  document.documentElement.dataset.locale = locale;
  try { localStorage.setItem('aethercore.locale', locale); } catch { /* non-fatal */ }
}

function interpolate(template: string, vars: Record<string, Primitive>): string {
  return Object.entries(vars).reduce((value, [name, replacement]) => value.replaceAll(`{${name}}`, String(replacement)), template);
}

export function t<K extends MessageKey>(key: K, locale: Locale, ...args: Args<K>): string {
  return interpolate(catalogs[locale][key], (args[0] ?? {}) as Record<string, Primitive>);
}

/** For message keys selected from a typed semantic map at runtime. Literal call sites should prefer t(). */
export function td(key: MessageKey, locale: Locale, vars: Record<string, Primitive> = {}): string {
  return interpolate(catalogs[locale][key], vars);
}

export function hasMessageKey(value: string): value is MessageKey {
  return Object.prototype.hasOwnProperty.call(enCatalog, value);
}

export function tp(key: PluralMessageKey, locale: Locale, count: number): string {
  const category = new Intl.PluralRules(locale).select(count);
  if (locale === 'ar') {
    const entry = arPlurals[key];
    const template = entry[category as keyof typeof entry] ?? entry.other;
    return interpolate(template, { count });
  }
  const entry = enPlurals[key];
  const template = category === 'one' ? entry.one : entry.other;
  return interpolate(template, { count });
}

export function formatNumber(value: number, locale: Locale, options?: Intl.NumberFormatOptions): string {
  return new Intl.NumberFormat(locale === 'ar' ? 'ar-IQ' : 'en-US', options).format(value);
}

export function formatDateTime(value: number, locale: Locale): string {
  return value ? new Intl.DateTimeFormat(locale === 'ar' ? 'ar-IQ' : 'en-US', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value)) : '—';
}

export { type MessageKey, type PluralMessageKey };
