import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { registerHooks } from 'node:module';
import { compile } from 'svelte/compiler';
import { render } from 'svelte/server';
import { batteryRows } from '../src/features/diagnostics/measurement-rows.ts';
import { formatDateTime, td } from '../src/lib/i18n/index.ts';
registerHooks({ resolve(specifier, context, next) {
  try { return next(specifier, context); } catch (error) {
    if ((error as { code?: string }).code === 'ERR_UNSUPPORTED_DIR_IMPORT') return next(`${specifier}/index.ts`, context);
    throw error;
  }
}, load(url, context, next) {
  if (url.endsWith('.svelte')) return { format: 'module', shortCircuit: true,
    source: compile(readFileSync(new URL(url), 'utf8'), { filename: new URL(url).pathname, generate: 'server' }).js.code };
  return next(url, context);
} });
const { default: MeasurementRows } = await import('../src/features/diagnostics/MeasurementRows.svelte');
const battery = { stableId: 'b0', displayName: 'Battery', hasDesignCapacity: false, designCapacityMwh: 0,
  hasFullChargeCapacity: false, fullChargeCapacityMwh: 0, hasCycleCount: true, cycleCount: 5,
  hasDesignCapacityRelative: true, designCapacityRelative: 100,
  hasFullChargeCapacityRelative: true, fullChargeCapacityRelative: 87,
  coverage: { source: 'IOCTL_BATTERY_QUERY_INFORMATION', availability: 1, hasObservedUnixMs: true, observedUnixMs: 1000 } };
for (const locale of ['en', 'ar'] as const) {
  const show = (value = battery) => render(MeasurementRows, { props: { title: td('measurement.battery.title', locale), rows: batteryRows([value] as never, locale), locale } }).body;
  test(`${locale}: relative battery capacities retain units, separate design/full and do not become mWh or damage`, () => {
    const html = show();
    assert.equal(batteryRows([battery] as never, locale)[0].value, td('measurement.unit.relative' as never, locale, { value: '87' }));
    assert.ok(html.includes(td('measurement.battery.design', locale, { value: td('measurement.unit.relative' as never, locale, { value: '100' }) })));
    assert.ok(!html.includes('mWh') && !html.includes('مللي واط'), 'no absolute conversion');
    assert.ok(!html.includes('%'), 'capacity is not an inferred wear verdict');
  });
  test(`${locale}: missing/zero relative capacities and conflicting units stay unmeasured`, () => {
    for (const patch of [
      { hasFullChargeCapacityRelative: false }, { fullChargeCapacityRelative: 0 },
      { fullChargeCapacityRelative: -1 }, { fullChargeCapacityRelative: 1.5 },
      { fullChargeCapacityRelative: 4294967296 },
      { hasFullChargeCapacity: true, fullChargeCapacityMwh: 41000 },
    ]) {
      assert.equal(batteryRows([{ ...battery, ...patch }] as never, locale)[0].value, null);
    }
    assert.ok(!show({ ...battery, hasDesignCapacityRelative: false } as never).includes(td('measurement.battery.design', locale, { value: td('measurement.unit.relative' as never, locale, { value: '100' }) })));
  });
}
