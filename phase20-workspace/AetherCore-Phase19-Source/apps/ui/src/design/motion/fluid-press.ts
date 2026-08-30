import { readMotionPreferences, subscribeMotionPreferences } from './preferences';
import { SpringValue } from './spring';

export type FluidPressOptions = {
  pressedScale?: number;
  hysteresis?: number;
};

/** Svelte action: instant pointer-down feedback with an interruptible presentation-value spring. */
export function fluidPress(node: HTMLElement, options: FluidPressOptions = {}) {
  let config = { ...options };
  const scale = new SpringValue(1, { response: 0.22, damping: 1 });
  let activePointer: number | null = null;
  let armed = false;
  let suppressNextClick = false;
  let preferences = readMotionPreferences();

  const pressedScale = () => Math.min(1, Math.max(0.8, config.pressedScale ?? 0.975));
  const hysteresis = () => Math.min(64, Math.max(0, config.hysteresis ?? 10));

  node.setAttribute('data-fluid-press', '');

  const unsubscribe = scale.subscribe((value) => {
    if (preferences.reducedMotion) {
      node.style.removeProperty('transform');
      return;
    }
    node.style.transform = `translateZ(0) scale(${value.toFixed(5)})`;
  });

  const setArmed = (next: boolean) => {
    armed = next;
    node.toggleAttribute('data-pressed', next);
    if (preferences.reducedMotion) scale.set(1);
    else scale.retarget(next ? pressedScale() : 1);
  };

  const unsubscribePreferences = subscribeMotionPreferences((next) => {
    preferences = next;
    if (preferences.reducedMotion) {
      node.style.removeProperty('transform');
      scale.set(1);
    } else {
      scale.retarget(armed ? pressedScale() : 1);
    }
  });

  const pointerdown = (event: PointerEvent) => {
    if (event.button !== 0 || activePointer !== null) return;
    suppressNextClick = false;
    activePointer = event.pointerId;
    node.setPointerCapture?.(event.pointerId);
    setArmed(true);
  };

  const pointermove = (event: PointerEvent) => {
    if (event.pointerId !== activePointer) return;
    const rect = node.getBoundingClientRect();
    const padding = hysteresis();
    const inside = event.clientX >= rect.left - padding
      && event.clientX <= rect.right + padding
      && event.clientY >= rect.top - padding
      && event.clientY <= rect.bottom + padding;
    if (inside !== armed) setArmed(inside);
  };

  const pointerup = (event: PointerEvent) => {
    if (event.pointerId !== activePointer) return;
    // Pointer capture intentionally keeps tracking alive outside the control. Without this click
    // fence, however, a captured pointer-up can still target the button after the user dragged
    // away to cancel. Preserve native activation only when the release remained armed.
    suppressNextClick = !armed;
    activePointer = null;
    setArmed(false);
  };

  const cancelPointer = (event: PointerEvent) => {
    if (event.pointerId !== activePointer) return;
    suppressNextClick = true;
    activePointer = null;
    setArmed(false);
  };

  const clickCapture = (event: MouseEvent) => {
    if (!suppressNextClick) return;
    suppressNextClick = false;
    event.preventDefault();
    event.stopImmediatePropagation();
  };

  const keydown = (event: KeyboardEvent) => {
    if (event.key === ' ' || event.key === 'Enter') {
      suppressNextClick = false;
      setArmed(true);
    }
  };
  const keyup = (event: KeyboardEvent) => {
    if (event.key === ' ' || event.key === 'Enter') setArmed(false);
  };

  node.addEventListener('pointerdown', pointerdown);
  node.addEventListener('pointermove', pointermove);
  node.addEventListener('pointerup', pointerup);
  node.addEventListener('pointercancel', cancelPointer);
  node.addEventListener('lostpointercapture', cancelPointer);
  node.addEventListener('click', clickCapture, true);
  node.addEventListener('keydown', keydown);
  node.addEventListener('keyup', keyup);
  const blur = () => setArmed(false);
  node.addEventListener('blur', blur);

  return {
    update(next: FluidPressOptions) {
      config = { ...next };
      if (armed && !preferences.reducedMotion) scale.retarget(pressedScale());
    },
    destroy() {
      unsubscribe();
      unsubscribePreferences();
      scale.destroy();
      node.removeEventListener('pointerdown', pointerdown);
      node.removeEventListener('pointermove', pointermove);
      node.removeEventListener('pointerup', pointerup);
      node.removeEventListener('pointercancel', cancelPointer);
      node.removeEventListener('lostpointercapture', cancelPointer);
      node.removeEventListener('click', clickCapture, true);
      node.removeEventListener('keydown', keydown);
      node.removeEventListener('keyup', keyup);
      node.removeEventListener('blur', blur);
      node.style.removeProperty('transform');
      node.removeAttribute('data-pressed');
      node.removeAttribute('data-fluid-press');
    },
  };
}
