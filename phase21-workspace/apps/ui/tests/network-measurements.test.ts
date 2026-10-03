import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { registerHooks } from 'node:module';
import { compile } from 'svelte/compiler';
import { render } from 'svelte/server';
import { networkRows } from '../src/features/diagnostics/measurement-rows.ts';
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
const counts = { inOctets: '18446744073709551615', outOctets: '0', inErrors: '0', outErrors: '0', inDiscards: '0', outDiscards: '0' };
const adapter = { stableId: 'guid', displayName: 'VPN', hasLinkSpeed: false, linkSpeedBps: 0,
  coverage: { source: 'GetAdaptersAddresses/GetIfEntry2', hasObservedUnixMs: true, observedUnixMs: 1000, availability: 1 },
  hasIsVirtual: true, isVirtual: true, hasAdminEnabled: true, adminEnabled: false,
  hasDefaultRouteV4: true, defaultRouteV4: false, hasDefaultRouteV6: true, defaultRouteV6: true,
  hasIpv4Apipa: true, ipv4Apipa: false, counters: counts, counterDelta: { elapsedMs: '1000', counts }, counterAvailability: 1, routeAvailability: 1 };
for (const locale of ['en', 'ar'] as const) {
  const show = (value = adapter) => render(MeasurementRows, { props: { title: td('measurement.network.title', locale), rows: networkRows([value] as never, locale), locale } }).body;
  test(`${locale}: owned real measurement rows show exact cumulative/delta values, disabled and IPv6 routes`, () => {
    const html = show();
    assert.ok(html.includes('18,446,744,073,709,551,615'));
    for (const key of ['measurement.network.adminDisabled', 'measurement.network.v6Default', 'measurement.network.virtual'])
      assert.ok(html.includes(td(key as never, locale)), key);
    assert.ok(!html.includes(td('measurement.network.noDefault' as never, locale)));
    assert.ok(html.includes('dir="ltr"'), 'adapter identity/source remain isolated');
  });
  test(`${locale}: first samples, failed reads, absent routes and malformed strings do not acquire rates or internet verdicts`, () => {
    let html = show({ ...adapter, counterDelta: null } as never);
    assert.ok(html.includes(td('measurement.network.twoSamples' as never, locale)));
    html = show({ ...adapter, counterAvailability: 5, routeAvailability: 5 } as never);
    assert.ok(html.includes(td('measurement.network.countersUnavailable' as never, locale)));
    assert.ok(html.includes(td('measurement.network.routesUnavailable' as never, locale)));
    assert.ok(!html.includes('18,446,744,073,709,551,615'));
    html = show({ ...adapter, counters: { ...counts, inOctets: '18446744073709551616' }, counterDelta: { elapsedMs: '0', counts }, defaultRouteV6: false } as never);
    assert.ok(html.includes(td('measurement.network.countersUnavailable' as never, locale)));
    assert.ok(html.includes(td('measurement.network.twoSamples' as never, locale)));
    assert.ok(html.includes(td('measurement.network.noDefault' as never, locale)));
  });
  test(`${locale}: recorded counter windows keep their reading date; absent dates and unknown states stay unknown`, () => {
    const row = networkRows([adapter] as never, locale)[0];
    assert.equal(row.observedUnixMs, 1000, 'no current-time substitution for a historical reading');
    assert.ok(show().includes(formatDateTime(1000, locale)));
    const missing = { ...adapter, coverage: { ...adapter.coverage, hasObservedUnixMs: false, availability: 0 } };
    const unknown = networkRows([missing] as never, locale)[0];
    assert.equal(unknown.observedUnixMs, null);
    assert.equal(unknown.availability, 'measurement.availability.unknown');
    const html = show(missing as never);
    assert.ok(html.includes(td('measurement.availability.unknown', locale)));
    assert.ok(!html.includes(formatDateTime(1000, locale)), 'no filler date is rendered');
  });
}
