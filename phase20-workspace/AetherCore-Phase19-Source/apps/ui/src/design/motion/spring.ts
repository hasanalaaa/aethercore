export type SpringConfig = {
  /** Designer-facing response in seconds. Lower is faster. */
  response: number;
  /** Damping ratio. 1 = critical, <1 = controlled overshoot. */
  damping: number;
  restSpeed: number;
  restDelta: number;
};

export const SPRING_DEFAULT: Readonly<SpringConfig> = Object.freeze({
  response: 0.36,
  damping: 1,
  restSpeed: 0.001,
  restDelta: 0.001,
});

export const SPRING_MOMENTUM: Readonly<SpringConfig> = Object.freeze({
  response: 0.34,
  damping: 0.82,
  restSpeed: 0.001,
  restDelta: 0.001,
});

export type MotionSubscriber = (value: number, velocity: number, settled: boolean) => void;

function finite(value: number, fallback: number): number {
  return Number.isFinite(value) ? value : fallback;
}

export class SpringValue {
  private current: number;
  private targetValue: number;
  private velocityValue = 0;
  private frame = 0;
  private lastTime = 0;
  private readonly subscribers = new Set<MotionSubscriber>();
  private config: SpringConfig;

  constructor(initialValue: number, config: Partial<SpringConfig> = {}) {
    this.current = finite(initialValue, 0);
    this.targetValue = this.current;
    this.config = { ...SPRING_DEFAULT, ...config };
  }

  get value(): number { return this.current; }
  get velocity(): number { return this.velocityValue; }
  get target(): number { return this.targetValue; }
  get running(): boolean { return this.frame !== 0; }

  subscribe(subscriber: MotionSubscriber): () => void {
    this.subscribers.add(subscriber);
    subscriber(this.current, this.velocityValue, !this.running);
    return () => this.subscribers.delete(subscriber);
  }

  configure(config: Partial<SpringConfig>): void {
    this.config = { ...this.config, ...config };
  }

  /** Hard synchronization for reduced-motion and external presentation-value adoption. */
  set(value: number): void {
    this.stop();
    this.current = finite(value, this.current);
    this.targetValue = this.current;
    this.velocityValue = 0;
    this.emit(true);
  }

  /** Adopt a directly manipulated presentation value without introducing a visual seam. */
  adoptPresentation(value: number, velocity = 0): void {
    this.stop();
    this.current = finite(value, this.current);
    this.targetValue = this.current;
    this.velocityValue = finite(velocity, 0);
    this.emit(Math.abs(this.velocityValue) <= this.config.restSpeed);
  }

  /**
   * Retarget from the live presentation value. Velocity is preserved unless a physical gesture
   * explicitly hands off a release velocity, which removes the drag→spring seam.
   */
  retarget(target: number, inheritedVelocity?: number): void {
    this.targetValue = finite(target, this.current);
    if (inheritedVelocity !== undefined) this.velocityValue = finite(inheritedVelocity, this.velocityValue);
    if (this.isSettled()) {
      this.current = this.targetValue;
      this.velocityValue = 0;
      this.emit(true);
      return;
    }
    if (!this.frame) {
      this.lastTime = performance.now();
      this.frame = requestAnimationFrame(this.tick);
    }
  }

  stop(): void {
    if (this.frame) cancelAnimationFrame(this.frame);
    this.frame = 0;
    this.lastTime = 0;
  }

  destroy(): void {
    this.stop();
    this.subscribers.clear();
  }

  private isSettled(): boolean {
    return Math.abs(this.velocityValue) <= this.config.restSpeed
      && Math.abs(this.targetValue - this.current) <= this.config.restDelta;
  }

  private emit(settled: boolean): void {
    for (const subscriber of this.subscribers) subscriber(this.current, this.velocityValue, settled);
  }

  private tick = (time: number): void => {
    const elapsed = Math.min(1 / 30, Math.max(0, (time - this.lastTime) / 1000));
    this.lastTime = time;

    // Semi-implicit Euler with small substeps is stable across 60/120/144 Hz and resumes safely
    // after a temporarily stalled renderer without integrating a giant frame delta.
    const stepMax = 1 / 120;
    const steps = Math.max(1, Math.ceil(elapsed / stepMax));
    const dt = elapsed / steps;
    const response = Math.max(0.08, this.config.response);
    const omega = (2 * Math.PI) / response;
    const damping = Math.max(0, this.config.damping);

    for (let index = 0; index < steps; index += 1) {
      const displacement = this.current - this.targetValue;
      const acceleration = -(omega * omega) * displacement - 2 * damping * omega * this.velocityValue;
      this.velocityValue += acceleration * dt;
      this.current += this.velocityValue * dt;
    }

    if (this.isSettled()) {
      this.current = this.targetValue;
      this.velocityValue = 0;
      this.frame = 0;
      this.emit(true);
      return;
    }

    this.emit(false);
    this.frame = requestAnimationFrame(this.tick);
  };
}
