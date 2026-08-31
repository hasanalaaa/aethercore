import { getCurrentWindow } from '@tauri-apps/api/window';
import { tauriAvailable } from '../platform/transport';

export type WindowUxCleanup = () => void;

function writeScale(scaleFactor: number): void {
  const root = document.documentElement;
  root.style.setProperty('--ac-display-scale', String(scaleFactor));
  root.dataset.scale = scaleFactor >= 2 ? '200' : scaleFactor >= 1.5 ? '150' : scaleFactor >= 1.25 ? '125' : '100';
}

export async function initializeWindowUx(): Promise<WindowUxCleanup> {
  const root = document.documentElement;
  const mediaReduce = window.matchMedia('(prefers-reduced-motion: reduce)');
  const mediaTransparency = window.matchMedia('(prefers-reduced-transparency: reduce)');
  const mediaContrast = window.matchMedia('(prefers-contrast: more)');
  const mediaForcedColors = window.matchMedia('(forced-colors: active)');

  const syncMedia = () => {
    root.dataset.motion = mediaReduce.matches ? 'reduced' : 'full';
    root.dataset.transparency = mediaTransparency.matches ? 'reduced' : 'full';
    root.dataset.contrast = mediaContrast.matches ? 'more' : 'normal';
    root.dataset.forcedColors = mediaForcedColors.matches ? 'active' : 'inactive';
  };

  syncMedia();
  mediaReduce.addEventListener('change', syncMedia);
  mediaTransparency.addEventListener('change', syncMedia);
  mediaContrast.addEventListener('change', syncMedia);
  mediaForcedColors.addEventListener('change', syncMedia);

  let unscale: (() => void) | undefined;
  let untheme: (() => void) | undefined;
  if (tauriAvailable) try {
    const current = getCurrentWindow();
    writeScale(await current.scaleFactor());
    unscale = await current.onScaleChanged(({ payload }) => writeScale(payload.scaleFactor));
    untheme = await current.onThemeChanged(({ payload }) => { root.dataset.systemTheme = payload ?? 'system'; });
  } catch {
    writeScale(window.devicePixelRatio || 1);
  }

  return () => {
    mediaReduce.removeEventListener('change', syncMedia);
    mediaTransparency.removeEventListener('change', syncMedia);
    mediaContrast.removeEventListener('change', syncMedia);
    mediaForcedColors.removeEventListener('change', syncMedia);
    unscale?.();
    untheme?.();
  };
}
