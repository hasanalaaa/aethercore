// Quality pass (lane 2): the Performance page must say what it measured and offer nothing it cannot do.
// 1. Every finding the bottleneck rules can raise has a title and summary in both languages. They had
//    none: the first finding made the page throw (`undefined.replaceAll`) instead of showing it.
// 2. Evidence reads as a sentence with units, not as `cpu.busyBp.avg: 9400 / 9000`.
// 3. No checkbox/tray/"review plan" that leads nowhere (the service refuses to execute any plan).
// Run: node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { hasMessageKey, td } from '../src/lib/i18n/index.ts';
import { enCatalog } from '../src/lib/i18n/catalog.en.ts';
import { arCatalog } from '../src/lib/i18n/catalog.ar.ts';

const arabic = /[؀-ۿ]/;
const rules = readFileSync(new URL('../../../crates/performance-bottleneck/src/lib.rs', import.meta.url), 'utf8');
const source = (path: string) => readFileSync(new URL(path, import.meta.url), 'utf8');

// The placeholders each summary is given by the Rust rule (read from `args: vec![arg("name", ...)]`).
const SUMMARY_ARGS: Record<string, string[]> = {
  'perf.finding.cpuSaturation.summary': ['averagePercent', 'peakPercent', 'sampleCount'],
  'perf.finding.dpcPressure.summary': ['dpcIsrPercent'],
  'perf.finding.powerClamp.summary': ['throttledSamples'],
  'perf.finding.thermalClamp.summary': ['throttledSamples'],
  'perf.finding.frequencyLimit.summary': ['throttledSamples'],
  'perf.finding.standbyStarvation.summary': ['hardFaultsAvg', 'hardFaultsPeak'],
  'perf.finding.commitPressure.summary': ['commitPressurePercent'],
  'perf.finding.ioSaturation.summary': ['activePeakPercent', 'activeAvgPercent'],
  'perf.finding.gpuBound.summary': ['gpuPeakPercent'],
  'perf.finding.workingSetBloat.summary': ['growthMb'],
};

const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

test('every finding message key the rules can raise is in the table above, so a new rule must come here', () => {
  const keys = [...rules.matchAll(/"(perf\.finding\.\w+\.(?:title|summary))"/g)].map((m) => m[1]);
  assert.ok(keys.length >= 20, `${keys.length} keys read from the rules`);
  for (const key of keys) {
    if (key.endsWith('.summary')) assert.ok(key in SUMMARY_ARGS, `${key} has no entry in SUMMARY_ARGS`);
  }
  for (const key of Object.keys(SUMMARY_ARGS)) assert.ok(keys.includes(key), `${key} is not raised by the rules any more`);
});

test('every finding has a title and a summary in English and Arabic, with exactly the placeholders its rule supplies', () => {
  const keys = [...new Set([...rules.matchAll(/"(perf\.finding\.\w+\.(?:title|summary))"/g)].map((m) => m[1]))];
  for (const key of keys) {
    assert.ok(hasMessageKey(key), `${key} is missing from the catalogs`);
    const en = (enCatalog as Record<string, string>)[key];
    const ar = (arCatalog as Record<string, string>)[key];
    assert.ok(en && en.length > 3, `${key} en`);
    assert.match(ar, arabic, `${key} ar`);
    if (key.endsWith('.summary')) {
      assert.deepEqual(placeholders(en), [...SUMMARY_ARGS[key]].sort(), `${key} en placeholders`);
      assert.deepEqual(placeholders(ar), [...SUMMARY_ARGS[key]].sort(), `${key} ar placeholders`);
    } else {
      assert.deepEqual(placeholders(en), [], `${key} title takes no arguments`);
    }
  }
});

test('every evidence fact the rules cite has a plain-language label in both languages', () => {
  const facts = [...new Set([...rules.matchAll(/"((?:cpu|memory|storage|gpu|power)\.[A-Za-z.]+)"/g)].map((m) => m[1]))];
  assert.ok(facts.length >= 9, `${facts}`);
  for (const fact of facts) {
    const key = `perf.fact.${fact}`;
    assert.ok(hasMessageKey(key), `${key} is missing`);
    assert.match(td(key as never, 'ar'), arabic, key);
  }
});

test('evidence reads as a sentence with units and never shows the raw fact key', async () => {
  const { formatEvidence } = await import('../src/features/performance/evidence.ts');
  const cpu = formatEvidence({ factKey: 'cpu.busyBp.avg', observedValue: 9400, threshold: 9000, observedUnixMs: 1 }, 'en');
  assert.match(cpu, /94%/);
  assert.match(cpu, /90%/);
  assert.doesNotMatch(cpu, /busyBp|9400|9000/);
  const latency = formatEvidence({ factKey: 'storage.transferLatencyUs', observedValue: 31000, threshold: 25000, observedUnixMs: 1 }, 'en');
  assert.match(latency, /31 ms/);
  assert.match(latency, /25 ms/);
  const growth = formatEvidence({ factKey: 'memory.modifiedList.growthBytes', observedValue: 805306368, threshold: 536870912, observedUnixMs: 1 }, 'en');
  assert.match(growth, /768 MB/);
  assert.match(growth, /512 MB/);
  const faults = formatEvidence({ factKey: 'memory.hardFaults.avg', observedValue: 820, threshold: 500, observedUnixMs: 1 }, 'en');
  assert.match(faults, /820/);
  assert.match(faults, /500/);
  const clamp = formatEvidence({ factKey: 'power.throttleActive', observedValue: 1, threshold: 0, observedUnixMs: 1 }, 'en');
  assert.doesNotMatch(clamp, /throttleActive|\b1\b|\b0\b/);
  const ar = formatEvidence({ factKey: 'cpu.busyBp.avg', observedValue: 9400, threshold: 9000, observedUnixMs: 1 }, 'ar');
  assert.match(ar, arabic);
  assert.doesNotMatch(ar, /busyBp/);
});

test('an evidence fact nobody labelled is shown as a number pair, never as an exception', async () => {
  const { formatEvidence } = await import('../src/features/performance/evidence.ts');
  const text = formatEvidence({ factKey: 'cpu.futureCounter', observedValue: 7, threshold: 5, observedUnixMs: 1 }, 'en');
  assert.match(text, /7/);
  assert.match(text, /5/);
});

test('the page offers no selection, tray or plan review that leads nowhere, and links to Startup', () => {
  const page = source('../src/features/performance/PerformancePage.svelte');
  const controller = source('../src/features/performance/controller.ts');
  assert.doesNotMatch(page, /type="checkbox"/);
  assert.doesNotMatch(page, /selection-tray|reviewOptimizationPlan|closeOptimizationReview/);
  assert.doesNotMatch(controller, /create_optimization_plan|reviewOptimizationPlan|closeOptimizationReview/);
  assert.match(page, /setPage\('startup'\)/);
});

// P3: an analysis error belongs to the monitoring run that produced it.
test('starting monitoring again clears the previous analysis error', () => {
  const controller = source('../src/features/performance/controller.ts');
  const start = controller.slice(controller.indexOf('export async function startPerfSampling'), controller.indexOf('export async function stopPerfSampling'));
  assert.match(start, /perfUi\.set\(\{ analysisError: '' \}\)/);
});
