const assert = require('node:assert/strict');
const path = require('node:path');

const compiled = process.argv[2];
if (!compiled) throw new Error('compiled motion output directory is required');
const physics = require(path.join(compiled, 'physics.js'));

assert(Math.abs(physics.projectMomentum(1000, .998) - 499) < 1e-9, 'Apple-style exponential projection changed');
assert(Math.abs(physics.projectedEndpoint(50, 1000, .998) - 549) < 1e-9, 'projected endpoint changed');
assert.equal(physics.nearestSnapPoint(122, [0, 100, 200]), 100);
assert(physics.rubberband(50, 400) > 0 && physics.rubberband(50, 400) < 50);
assert(physics.rubberband(-50, 400) < 0 && physics.rubberband(-50, 400) > -50);
assert(Math.abs(physics.estimateVelocity([{ position: 0, time: 0 }, { position: 50, time: 50 }]) - 1000) < 1e-9);

let now = 0;
let id = 0;
const frames = new Map();
global.requestAnimationFrame = (callback) => { const next = ++id; frames.set(next, callback); return next; };
global.cancelAnimationFrame = (frame) => frames.delete(frame);
Object.defineProperty(global, 'performance', { value: { now: () => now }, configurable: true });
const { SpringValue } = require(path.join(compiled, 'spring.js'));

function step(ms = 1000 / 120) {
  now += ms;
  const pending = [...frames.entries()];
  frames.clear();
  for (const [, callback] of pending) callback(now);
}
function settle(maxFrames = 1200) {
  for (let index = 0; index < maxFrames && frames.size; index += 1) step();
  assert.equal(frames.size, 0, 'spring did not settle within deterministic frame budget');
}

const critical = new SpringValue(0, { response: .36, damping: 1, restDelta: 1e-5, restSpeed: 1e-5 });
let maxValue = 0;
critical.subscribe((value) => { maxValue = Math.max(maxValue, value); });
critical.retarget(1);
settle();
assert(Math.abs(critical.value - 1) < 1e-9);
assert(maxValue <= 1.000001, `critically damped spring overshot: ${maxValue}`);

const handoff = new SpringValue(20, { response: .34, damping: .82, restDelta: 1e-4, restSpeed: 1e-4 });
handoff.adoptPresentation(42, 600);
assert.equal(handoff.value, 42);
assert.equal(handoff.velocity, 600);
handoff.retarget(100, 600);
step();
assert(handoff.value > 42, 'release velocity did not continue into spring motion');
const presentationAtInterrupt = handoff.value;
handoff.retarget(0);
assert.equal(handoff.value, presentationAtInterrupt, 'retarget jumped away from live presentation value');
settle();
assert(Math.abs(handoff.value) < 1e-9);

console.log('Phase 11 motion physics tests: PASS');

// Omega frame-pacing convergence: exercise the same integrator at representative desktop refresh
// rates and after a renderer stall. This is deterministic numerical evidence, not a GPU frame-time
// or WebView2 qualification claim.
function resetClock() {
  for (const frame of [...frames.keys()]) cancelAnimationFrame(frame);
  now = 0;
}

function settleAtHz(hz, spring, maxSeconds = 8) {
  const frameMs = 1000 / hz;
  const maxFrames = Math.ceil(hz * maxSeconds);
  for (let index = 0; index < maxFrames && frames.size; index += 1) step(frameMs);
  assert.equal(frames.size, 0, `spring did not settle at ${hz} Hz`);
  assert(Number.isFinite(spring.value) && Number.isFinite(spring.velocity), `non-finite spring state at ${hz} Hz`);
}

for (const hz of [60, 120, 144]) {
  resetClock();
  const sample = new SpringValue(0, { response: .36, damping: 1, restDelta: 1e-5, restSpeed: 1e-5 });
  let previous = sample.value;
  let observedMax = sample.value;
  sample.subscribe((value) => {
    assert(Number.isFinite(value), `non-finite presentation value at ${hz} Hz`);
    // A critically damped monotonic move should never reverse or overshoot under the integrator.
    assert(value + 1e-8 >= previous, `critical spring reversed at ${hz} Hz: ${previous} -> ${value}`);
    previous = value;
    observedMax = Math.max(observedMax, value);
  });
  sample.retarget(1);
  settleAtHz(hz, sample);
  assert(Math.abs(sample.value - 1) < 1e-9, `spring missed target at ${hz} Hz`);
  assert(observedMax <= 1.000001, `critical spring overshot at ${hz} Hz: ${observedMax}`);

  resetClock();
  const velocityCarry = new SpringValue(10, { response: .34, damping: .82, restDelta: 1e-4, restSpeed: 1e-4 });
  velocityCarry.adoptPresentation(40, 720);
  velocityCarry.retarget(100, 720);
  const beforeReleaseFrame = velocityCarry.value;
  step(1000 / hz);
  assert(velocityCarry.value > beforeReleaseFrame, `release velocity seam at ${hz} Hz`);
  const live = velocityCarry.value;
  const liveVelocity = velocityCarry.velocity;
  velocityCarry.retarget(0);
  assert.equal(velocityCarry.value, live, `retarget presentation discontinuity at ${hz} Hz`);
  assert.equal(velocityCarry.velocity, liveVelocity, `retarget discarded live velocity at ${hz} Hz`);
  settleAtHz(hz, velocityCarry);
}

resetClock();
const stalled = new SpringValue(0, { response: .36, damping: 1, restDelta: 1e-5, restSpeed: 1e-5 });
stalled.retarget(1);
step(250); // tick() must clamp this to 1/30 s rather than integrating an unstable 250 ms delta.
assert(Number.isFinite(stalled.value) && Number.isFinite(stalled.velocity), 'renderer stall destabilized spring state');
assert(stalled.value >= 0 && stalled.value <= 1.000001, `renderer stall caused invalid critical response: ${stalled.value}`);
settleAtHz(60, stalled);

console.log('Omega motion frame-rate/stall tests: PASS (60/120/144 Hz numerical integration)');
