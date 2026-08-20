export type MotionPreferences = {
  reducedMotion: boolean;
  reducedTransparency: boolean;
  increasedContrast: boolean;
};

export type MotionPreferencesSubscriber = (preferences: MotionPreferences) => void;

const FALLBACK: MotionPreferences = {
  reducedMotion: false,
  reducedTransparency: false,
  increasedContrast: false,
};

let current: MotionPreferences = FALLBACK;
let initialized = false;
const subscribers = new Set<MotionPreferencesSubscriber>();
let reducedMotionQuery: MediaQueryList | null = null;
let reducedTransparencyQuery: MediaQueryList | null = null;
let increasedContrastQuery: MediaQueryList | null = null;

function same(a: MotionPreferences, b: MotionPreferences): boolean {
  return a.reducedMotion === b.reducedMotion
    && a.reducedTransparency === b.reducedTransparency
    && a.increasedContrast === b.increasedContrast;
}

function calculate(): MotionPreferences {
  if (typeof document === 'undefined') return FALLBACK;
  const root = document.documentElement;
  return {
    reducedMotion: root.dataset.motion === 'reduced' || Boolean(reducedMotionQuery?.matches),
    reducedTransparency: root.dataset.transparency === 'reduced' || Boolean(reducedTransparencyQuery?.matches),
    increasedContrast: root.dataset.contrast === 'more' || Boolean(increasedContrastQuery?.matches),
  };
}

function refresh(): void {
  const next = calculate();
  if (same(current, next)) return;
  current = next;
  for (const subscriber of subscribers) subscriber({ ...current });
}

function initialize(): void {
  if (initialized || typeof window === 'undefined' || typeof document === 'undefined') return;
  initialized = true;
  reducedMotionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
  reducedTransparencyQuery = window.matchMedia('(prefers-reduced-transparency: reduce)');
  increasedContrastQuery = window.matchMedia('(prefers-contrast: more)');
  for (const query of [reducedMotionQuery, reducedTransparencyQuery, increasedContrastQuery]) {
    query.addEventListener('change', refresh);
  }
  new MutationObserver(refresh).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-motion', 'data-transparency', 'data-contrast'],
  });
  current = calculate();
}

/** Cached accessibility preferences. No matchMedia objects are allocated in animation hot paths. */
export function readMotionPreferences(): MotionPreferences {
  initialize();
  return { ...current };
}

/** App-lifetime preference stream used by motion primitives to react immediately to OS changes. */
export function subscribeMotionPreferences(subscriber: MotionPreferencesSubscriber): () => void {
  initialize();
  subscribers.add(subscriber);
  subscriber({ ...current });
  return () => subscribers.delete(subscriber);
}
