const path = require('node:path');
const { performance } = require('node:perf_hooks');
const compiled = process.argv[2];
if (!compiled) throw new Error('compiled motion output directory is required');
const physics = require(path.join(compiled, 'physics.js'));

const iterations = Number(process.env.AETHERCORE_MOTION_BENCH_ITERS || 1_000_000);
let sink = 0;
const started = performance.now();
for (let i = 0; i < iterations; i += 1) {
  const velocity = (i % 4001) - 2000;
  const projected = physics.projectedEndpoint(i % 500, velocity, .998);
  sink += physics.nearestSnapPoint(projected, [0, 100, 200, 300, 400, 500]);
  sink += physics.rubberband(velocity / 10, 400);
}
const elapsedMs = performance.now() - started;
const payload = {
  schema: 'aethercore.motion-benchmark.v1',
  scope: 'platform-neutral TypeScript motion math compiled from repository source',
  iterations,
  elapsed_ms: Number(elapsedMs.toFixed(3)),
  ns_per_iteration: Number(((elapsedMs * 1e6) / iterations).toFixed(3)),
  checksum: Number(sink.toFixed(6)),
  qualification_note: 'Microbenchmark is comparative source evidence only; it is not WebView2 frame-time or GPU pacing evidence.'
};
console.log(JSON.stringify(payload));
