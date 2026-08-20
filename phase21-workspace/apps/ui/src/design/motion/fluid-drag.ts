import { estimateVelocity, nearestSnapPoint, projectedEndpoint, rubberband, type VelocitySample } from './physics';
import { readMotionPreferences, subscribeMotionPreferences } from './preferences';
import { SPRING_MOMENTUM, SpringValue } from './spring';

export type FluidDragAxis = 'x' | 'y';
export type FluidDragOptions = {
  axis?: FluidDragAxis;
  initial?: number;
  min?: number;
  max?: number;
  snapPoints?: readonly number[];
  decelerationRate?: number;
  rubberbandDimension?: number;
  onPosition?: (value: number, settled: boolean) => void;
};

/**
 * Direct-manipulation action for the few AetherCore surfaces where dragging is meaningful.
 * It tracks the pointer 1:1, softens bounds, estimates release velocity, projects momentum,
 * and hands the exact release velocity into an interruptible spring.
 */
export function fluidDrag(node: HTMLElement, options: FluidDragOptions = {}) {
  let config = { ...options };
  const axis = () => config.axis ?? 'y';
  const spring = new SpringValue(config.initial ?? 0, SPRING_MOMENTUM);
  let activePointer: number | null = null;
  let pointerOrigin = 0;
  let valueOrigin = spring.value;
  let samples: VelocitySample[] = [];
  let preferences = readMotionPreferences();

  node.setAttribute('data-fluid-drag', '');

  const render = (value: number, _velocity: number, settled: boolean) => {
    if (axis() === 'x') node.style.translate = `${value.toFixed(3)}px 0`;
    else node.style.translate = `0 ${value.toFixed(3)}px`;
    config.onPosition?.(value, settled);
  };
  const unsubscribe = spring.subscribe(render);
  const unsubscribePreferences = subscribeMotionPreferences((next) => {
    preferences = next;
    if (preferences.reducedMotion && activePointer === null) spring.set(spring.target);
  });

  const positionOf = (event: PointerEvent) => axis() === 'x' ? event.clientX : event.clientY;

  function bounded(raw: number): number {
    const min = config.min ?? Number.NEGATIVE_INFINITY;
    const max = config.max ?? Number.POSITIVE_INFINITY;
    const dimension = config.rubberbandDimension ?? Math.max(80, node.getBoundingClientRect()[axis() === 'x' ? 'width' : 'height']);
    if (raw < min) return min + rubberband(raw - min, dimension);
    if (raw > max) return max + rubberband(raw - max, dimension);
    return raw;
  }

  function record(position: number, time: number) {
    samples.push({ position, time });
    const cutoff = time - 120;
    while (samples.length > 2 && samples[0].time < cutoff) samples.shift();
  }

  const pointerdown = (event: PointerEvent) => {
    if (event.button !== 0 || activePointer !== null) return;
    activePointer = event.pointerId;
    pointerOrigin = positionOf(event);
    valueOrigin = spring.value;
    samples = [];
    record(valueOrigin, event.timeStamp);
    spring.stop();
    node.setPointerCapture?.(event.pointerId);
    node.toggleAttribute('data-dragging', true);
  };

  const pointermove = (event: PointerEvent) => {
    if (event.pointerId !== activePointer) return;
    const next = bounded(valueOrigin + positionOf(event) - pointerOrigin);
    record(next, event.timeStamp);
    const velocity = estimateVelocity(samples);
    spring.adoptPresentation(next, velocity);
  };

  const settle = (event: PointerEvent, preserveMomentum: boolean) => {
    if (event.pointerId !== activePointer) return;
    const velocity = preserveMomentum ? estimateVelocity(samples) : 0;
    activePointer = null;
    node.toggleAttribute('data-dragging', false);

    const min = config.min ?? Number.NEGATIVE_INFINITY;
    const max = config.max ?? Number.POSITIVE_INFINITY;
    const projected = projectedEndpoint(spring.value, velocity, config.decelerationRate ?? 0.998);
    const candidates = config.snapPoints?.length
      ? config.snapPoints.filter((value) => value >= min && value <= max)
      : [Math.max(min, Math.min(max, projected))];
    const target = nearestSnapPoint(projected, candidates.length ? candidates : [spring.value]);

    if (preferences.reducedMotion) spring.set(target);
    else spring.retarget(target, velocity);
  };

  const release = (event: PointerEvent) => settle(event, true);
  const cancel = (event: PointerEvent) => settle(event, false);

  node.addEventListener('pointerdown', pointerdown);
  node.addEventListener('pointermove', pointermove);
  node.addEventListener('pointerup', release);
  node.addEventListener('pointercancel', cancel);
  node.addEventListener('lostpointercapture', cancel);

  return {
    update(next: FluidDragOptions) {
      config = { ...next };
      if (activePointer === null && typeof next.initial === 'number' && next.initial !== spring.target) {
        if (preferences.reducedMotion) spring.set(next.initial);
        else spring.retarget(next.initial);
      }
    },
    destroy() {
      unsubscribe();
      unsubscribePreferences();
      spring.destroy();
      node.removeEventListener('pointerdown', pointerdown);
      node.removeEventListener('pointermove', pointermove);
      node.removeEventListener('pointerup', release);
      node.removeEventListener('pointercancel', cancel);
      node.removeEventListener('lostpointercapture', cancel);
      node.style.removeProperty('translate');
      node.removeAttribute('data-fluid-drag');
      node.removeAttribute('data-dragging');
    },
  };
}
