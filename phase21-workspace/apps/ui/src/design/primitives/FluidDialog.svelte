<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { readMotionPreferences, subscribeMotionPreferences, SpringValue } from '../motion';

  export let open = false;
  export let labelledBy: string;
  export let className = '';
  export let backdropClassName = '';
  export let onClose: () => void = () => {};
  export let onClosed: () => void = () => {};
  export let closeOnBackdrop = true;
  export let transformOrigin = '50% 12%';

  let rendered = open;
  let progress = open ? 1 : 0;
  let latestOpen = open;
  let dialog: HTMLElement | null = null;
  $: visualProgress = Math.max(0, Math.min(1, progress));
  $: dialogOpacity = visualProgress.toFixed(4);
  $: dialogTranslate = ((1 - visualProgress) * 10).toFixed(3);
  $: dialogScale = (0.985 + visualProgress * 0.015).toFixed(5);
  $: scrimAlpha = (0.58 * visualProgress).toFixed(4);
  $: backdropBlur = `${(16 * visualProgress).toFixed(2)}px`;
  $: surfaceBlur = `${(18 + 24 * visualProgress).toFixed(2)}px`;
  let returnFocus: HTMLElement | null = null;

  function completeClose(): void {
    // The component stays mounted while its content is not rendered. Guarding on `rendered`
    // prevents preference changes or repeated settled notifications from firing onClosed twice.
    if (!rendered) return;
    rendered = false;
    const target = returnFocus;
    returnFocus = null;
    onClosed();
    requestAnimationFrame(() => target?.focus({ preventScroll: true }));
  }

  const spring = new SpringValue(progress, { response: 0.30, damping: 1, restDelta: 0.001, restSpeed: 0.001 });
  let reducedMotion = readMotionPreferences().reducedMotion;
  const unsubscribe = spring.subscribe((value, _velocity, settled) => {
    progress = value;
    if (settled && !latestOpen && value <= 0.001) {
      completeClose();
    }
  });

  const unsubscribePreferences = subscribeMotionPreferences((preferences) => {
    reducedMotion = preferences.reducedMotion;
    if (reducedMotion) spring.set(latestOpen ? 1 : 0);
  });

  function focusables(): HTMLElement[] {
    if (!dialog) return [];
    return Array.from(dialog.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])'))
      .filter((element) => !element.hasAttribute('hidden') && element.getAttribute('aria-hidden') !== 'true');
  }

  async function focusDialog(): Promise<void> {
    await tick();
    if (!latestOpen || !dialog) return;
    const items = focusables();
    (items[0] ?? dialog).focus({ preventScroll: true });
  }

  $: if (open !== latestOpen) {
    latestOpen = open;
    if (open) {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      rendered = true;
      void focusDialog();
    }
    if (reducedMotion) {
      spring.set(open ? 1 : 0);
    } else {
      // Reversal retargets the live presentation value and inherits the current spring velocity.
      spring.retarget(open ? 1 : 0);
    }
  }

  function backdropDismiss(node: HTMLElement) {
    const pointerdown = (event: PointerEvent) => {
      if (!closeOnBackdrop || event.target !== event.currentTarget) return;
      onClose();
    };
    node.addEventListener('pointerdown', pointerdown);
    return { destroy: () => node.removeEventListener('pointerdown', pointerdown) };
  }

  function dialogKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== 'Tab') return;
    const items = focusables();
    if (!items.length) { event.preventDefault(); dialog?.focus(); return; }
    const first = items[0];
    const last = items[items.length - 1];
    const active = document.activeElement;
    if (event.shiftKey && (active === first || !dialog?.contains(active))) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && (active === last || !dialog?.contains(active))) { event.preventDefault(); first.focus(); }
  }

  onDestroy(() => { unsubscribe(); unsubscribePreferences(); spring.destroy(); });
</script>

{#if rendered}
  <div
    class={`ac-dialog-backdrop ${backdropClassName}`}
    role="presentation"
    style={`--ac-dialog-progress:${visualProgress};--ac-dialog-scrim-alpha:${scrimAlpha};--ac-dialog-blur:${backdropBlur}`}
    use:backdropDismiss
  >
    <section
      bind:this={dialog}
      class={`ac-fluid-dialog ac-material ac-material-focused ${className}`}
      role="dialog"
      aria-modal="true"
      aria-labelledby={labelledBy}
      tabindex="-1"
      onkeydown={dialogKeydown}
      style={`--ac-dialog-progress:${visualProgress};--ac-fluid-surface-blur:${surfaceBlur};opacity:${dialogOpacity};transform:translate3d(0,${dialogTranslate}px,0) scale(${dialogScale});transform-origin:${transformOrigin}`}
    >
      <slot />
    </section>
  </div>
{/if}
