export type VelocitySample = { position: number; time: number };

/** Apple's exponential scroll-style momentum projection, in CSS pixels. */
export function projectMomentum(initialVelocityPxPerSecond: number, decelerationRate = 0.998): number {
  const rate = Math.min(0.9999, Math.max(0.8, decelerationRate));
  return (initialVelocityPxPerSecond / 1000) * rate / (1 - rate);
}

export function projectedEndpoint(currentPosition: number, releaseVelocity: number, decelerationRate = 0.998): number {
  return currentPosition + projectMomentum(releaseVelocity, decelerationRate);
}

/** Progressive resistance rather than a frozen hard boundary. */
export function rubberband(overshoot: number, dimension: number, constant = 0.55): number {
  const safeDimension = Math.max(1, Math.abs(dimension));
  return (overshoot * safeDimension * constant) / (safeDimension + constant * Math.abs(overshoot));
}

export function estimateVelocity(samples: readonly VelocitySample[], horizonMs = 80): number {
  if (samples.length < 2) return 0;
  const newest = samples[samples.length - 1];
  let oldest = samples[0];
  for (let index = samples.length - 2; index >= 0; index -= 1) {
    const sample = samples[index];
    oldest = sample;
    if (newest.time - sample.time >= horizonMs) break;
  }
  const elapsed = Math.max(1, newest.time - oldest.time);
  return ((newest.position - oldest.position) / elapsed) * 1000;
}

export function nearestSnapPoint(projected: number, snapPoints: readonly number[]): number {
  if (!snapPoints.length) return projected;
  return snapPoints.reduce((best, point) => Math.abs(point - projected) < Math.abs(best - projected) ? point : best, snapPoints[0]);
}
