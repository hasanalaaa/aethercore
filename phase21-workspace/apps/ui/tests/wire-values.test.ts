// P75 trial run: values the service emits that the UI printed raw (English inside the Arabic UI).
// Run: node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  localizeCleanupKind, localizeConfidence, localizeDirection, localizeDomain, localizeFactState, localizeFleetCadence,
  localizeHealthStatus, localizeImpact, localizeKind, localizeMatchQuality, localizeMemoryPressure, localizeOwnedText,
  localizePlanKind, localizeRecommendation, localizeRecommendationReason, localizeRisk, localizeSeverity,
  localizeStartupScope, localizeState, td, tp,
} from '../src/lib/i18n/index.ts';
import * as names from '../src/lib/i18n/index.ts';

const arabic = /[؀-ۿ]/;

test('startup scope: the registry and startup-folder scopes the Windows provider emits', () => {
  // crates/startup-manager/src/windows_impl.rs: "Machine" (HKLM, ProgramData startup), "User" (HKU)
  for (const scope of ['Machine', 'User', 'Machine service', 'Scheduled task']) {
    assert.match(localizeStartupScope(scope, 'ar'), arabic, scope);
  }
});

test('driver match quality: every MatchKind label the hub emits', () => {
  // crates/driver-hub/src/lib.rs match_quality
  for (const quality of ['Hardware ID', 'Compatible ID', 'Vendor family', 'Device class', 'Unknown']) {
    assert.match(localizeMatchQuality(quality, 'ar'), arabic, quality);
  }
});

test('driver recommendation reasons: every RecommendationReason code', () => {
  // crates/driver-authority/src/lib.rs RecommendationReason::as_str
  for (const code of [
    'OEM_MACHINE_SPECIFIC', 'WINDOWS_APPLICABLE', 'COMPONENT_REFERENCE_DRIVER', 'INSTALLED_DRIVER_ALREADY_PREFERRED',
    'NEWER_BUT_LOWER_AUTHORITY', 'FIRMWARE_REQUIRES_MANUAL_REVIEW', 'EXACT_HARDWARE_ID_MATCH', 'COMPATIBLE_ID_MATCH',
    'MISSING_DRIVER_PRIORITY', 'DEVICE_PROBLEM_PRIORITY', 'TRUST_REJECTED', 'INCOMPATIBLE', 'VERSION_ORDERING_UNKNOWN',
    'AUTHORITY_COVERAGE_INCOMPLETE', 'USER_IGNORED_EXACT_VERSION', 'USER_IGNORED_OPTIONAL', 'USER_DEFERRED',
  ]) {
    assert.match(localizeRecommendationReason(code, 'ar'), arabic, code);
    assert.doesNotMatch(localizeRecommendationReason(code, 'en'), /_/, code);
  }
});

test('repair recovery facts: every FactState, as the service writes it (lowerCamel)', () => {
  // crates/windows-repair-intelligence/src/model.rs FactState via protocol.rs lower_camel_debug
  for (const state of [
    'healthy', 'repairable', 'corruptionDetected', 'repairFailed', 'sourceRequired', 'rebootRequired', 'active', 'stopped',
    'disabled', 'unexpectedConfiguration', 'unavailable', 'offline', 'failure', 'degraded', 'available', 'unknown',
  ]) {
    assert.match(localizeFactState(state, 'ar'), arabic, state);
    assert.notEqual(localizeFactState(state, 'en'), state, state);
  }
});

test('repair evidence count agrees with its number in Arabic', () => {
  assert.equal(tp('unit.evidenceItem', 'ar', 2), 'عنصرا دليل');
  assert.equal(tp('unit.evidenceItem', 'en', 1), '1 evidence item');
});

test('every insight key the model and rule paths emit resolves in both catalogs', async () => {
  // P75 review (#29): llama.rs emitted insight.summary.observation, which neither catalog had,
  // so every model insight showed its raw key. Read the keys from the Rust sources themselves,
  // so a key added there without a label fails here.
  const { readdirSync, readFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const { enCatalog } = await import('../src/lib/i18n/catalog.en.ts');
  const { arCatalog } = await import('../src/lib/i18n/catalog.ar.ts');
  const roots = ['../../../crates/intelligence-core/src', '../../../services/maintenance-service/src'].map((r) => join(import.meta.dirname, r));
  const keys = new Set<string>();
  for (const root of roots) {
    for (const file of readdirSync(root).filter((f) => f.endsWith('.rs'))) {
      for (const m of readFileSync(join(root, file), 'utf8').matchAll(/"(insight\.[a-zA-Z.]+)"/g)) keys.add(m[1]);
    }
  }
  assert.ok(keys.has('insight.summary.observation'), [...keys].join(', '));
  for (const key of keys) {
    assert.ok(key in enCatalog, `EN lacks ${key}`);
    assert.ok(key in arCatalog, `AR lacks ${key}`);
  }
});

test('driver target-version source: the two the hub emits', async () => {
  const { driverTargetEvidence } = await import('../src/lib/i18n/index.ts');
  for (const source of ['WuaMetadata', 'ProviderMetadata']) {
    const text = driverTargetEvidence({ targetVersion: '1.2.3', targetVersionSource: source } as never, 'ar');
    assert.match(text, arabic, source);
  }
});

test('fleet cadence as the desktop writes it', async () => {
  const { localizeFleetCadence } = await import('../src/lib/i18n/index.ts');
  assert.equal(localizeFleetCadence('every_hours:24', 'en'), 'every 24 hours');
  assert.equal(localizeFleetCadence('every_hours:2', 'ar'), 'كل ساعتين');
  assert.match(localizeFleetCadence('daily_at_utc_hour:3', 'ar'), arabic);
});

test('recovery record severities: "Amber" (cleaner, startup, repair) and "warning" (driver install)', async () => {
  const { localizeSeverity } = await import('../src/lib/i18n/index.ts');
  for (const severity of ['Amber', 'warning']) assert.match(localizeSeverity(severity, 'ar'), arabic, severity);
});

test('deep scan headline: a partial scan with no findings is not "Healthy"', async () => {
  const { deepScanHeadlineKey } = await import('../src/features/intelligence/headline.ts');
  assert.equal(deepScanHeadlineKey(1, 4), 'deepScan.status.partialClear');
  assert.equal(deepScanHeadlineKey(1, 3), 'deepScan.status.healthy');
  assert.equal(deepScanHeadlineKey(0, 3), 'common.unknown');
  assert.equal(deepScanHeadlineKey(3, 4), 'deepScan.status.action');
});

// P75 review 2: a scan whose collectors all failed has no findings, so the coordinator's
// status is Healthy; the scan and its history entry must not say so.
test('deep scan headline: a failed or cancelled scan is not "Healthy"', async () => {
  const { deepScanHeadlineKey } = await import('../src/features/intelligence/headline.ts');
  assert.equal(deepScanHeadlineKey(1, 5), 'deepScan.state.cancelled');
  assert.equal(deepScanHeadlineKey(1, 6), 'deepScan.state.failed');
});

// P76 DBT-P75-091: a scan still running has no findings yet, so its status reads Healthy;
// the headline says it is scanning, not a verdict on what has not been checked.
test('deep scan headline: a running scan is not "Healthy"', async () => {
  const { deepScanHeadlineKey } = await import('../src/features/intelligence/headline.ts');
  assert.equal(deepScanHeadlineKey(1, 2), 'deepScan.state.scanning');
  assert.equal(deepScanHeadlineKey(2, 2), 'deepScan.status.attention', 'a finding already made is shown');
});

test('disk activity a provider says it did not measure reads unmeasured, not 0%', async () => {
  const { healthChannels } = await import('../src/features/overview/instrument.ts');
  const { createInitialStreamState } = await import('../src/platform/stream-state.ts');
  const base = { ...createInitialStreamState().performance, capturedUnixMs: 1 };
  const device = { deviceId: '/', friendlyName: '/', activeTimeBp: 0, queueDepthX100: 0, avgTransferLatencyUs: 0, readBytesPerSec: 0, writeBytesPerSec: 0, totalSpaceBytes: 1, freeSpaceBytes: 1 };
  const macos = { ...base, storage: [device], collectorFaults: [{ collector: 'storage.activeTime', kind: 'Degraded', detail: 'not measured on macOS' }] };
  assert.equal(healthChannels(macos as never, 'en').find((c) => c.id === 'disk')?.pct, undefined);
  const windows = { ...base, storage: [{ ...device, activeTimeBp: 0 }], collectorFaults: [] };
  assert.equal(healthChannels(windows as never, 'en').find((c) => c.id === 'disk')?.pct, 0, 'a measured idle disk is 0');
});

// P77-01: the display boundary fails closed. An unknown enum value, or English prose from the
// backend, is not shown as itself in either language; what the app knows keeps its meaning;
// device names, paths, numbers and codes stay data.
test('an unknown enum value is never shown as itself, in either language', () => {
  const unknown = 'ZzFutureValue';
  const labels: Record<string, (value: string, locale: 'en' | 'ar') => string> = {
    state: localizeState, risk: localizeRisk, severity: localizeSeverity, confidence: localizeConfidence,
    impact: localizeImpact, kind: localizeKind, direction: localizeDirection, domain: localizeDomain,
    recommendation: localizeRecommendation, planKind: localizePlanKind, matchQuality: localizeMatchQuality,
    reason: localizeRecommendationReason, factState: localizeFactState, memoryPressure: localizeMemoryPressure,
    health: localizeHealthStatus, startupScope: localizeStartupScope, cleanupKind: localizeCleanupKind,
    cadence: localizeFleetCadence,
  };
  for (const [name, label] of Object.entries(labels)) {
    for (const locale of ['en', 'ar'] as const) {
      const shown = label(unknown, locale);
      assert.notEqual(shown, unknown, `${name} ${locale} printed the unknown value`);
      assert.equal(shown, td('common.unknown', locale), `${name} ${locale}`);
    }
  }
  assert.equal(localizeCleanupKind('', 'ar'), '', 'an empty kind has nothing to hide');
});

test('backend English prose is never shown in Arabic, not even inside an Arabic sentence', () => {
  const injected = ['Backend exploded', 'Unknown message', 'Injected English', 'kaboom happened', 'Another injected sentence'];
  for (const prose of [
    'Backend exploded',
    'Hardware telemetry: Backend exploded',
    'preflight rejected: Unknown message',
    'Storage telemetry unavailable: Injected English',
    'WHEA event 7: Injected English',
    'Windows Update execution failed: Another injected sentence',
    'Crash diagnostics: kaboom happened',
  ]) {
    const shown = localizeOwnedText(prose, 'ar');
    assert.equal(shown.localized, true, `${prose} was handed to the technical-text path`);
    assert.match(shown.text, arabic, prose);
    for (const word of injected) assert.ok(!shown.text.includes(word), `${prose} → ${shown.text}`);
  }
});

test('known owned messages keep their meaning, and data is not damaged', () => {
  const known = localizeOwnedText('preflight rejected: device inventory changed', 'ar');
  assert.match(known.text, arabic);
  assert.ok(!known.text.includes('device inventory changed'), known.text);
  assert.notEqual(known.text, localizeOwnedText('preflight rejected: Unknown message', 'ar').text,
    'a known detail must not read as the unavailable text');
  // Device names, paths, numbers and codes are data, not product prose.
  assert.ok(localizeOwnedText('Samsung 980 — storage reliability', 'ar').text.includes('Samsung 980'));
  assert.ok(localizeOwnedText('Dump: C:\\Windows\\Minidump\\052624-1.dmp', 'ar').text.includes('C:\\Windows\\Minidump\\052624-1.dmp'));
  assert.ok(localizeOwnedText('Bugcheck: 0x0000009F', 'ar').text.includes('0x0000009F'));
  assert.ok(localizeOwnedText('PnP inventory failed: 0x80070005', 'ar').text.includes('0x80070005'));
  assert.ok(localizeOwnedText('Windows Update search did not complete successfully: 0x8024402C', 'ar').text.includes('0x8024402C'));
  // The English UI keeps English prose readable.
  assert.equal(localizeOwnedText('Hardware telemetry: Backend exploded', 'en').text, 'Hardware telemetry: Backend exploded');
});

test('a caller that shows data opts out of the fallback, and only then', () => {
  const command = 'C:\\Program Files\\Vendor App\\app.exe --background';
  assert.deepEqual(localizeOwnedText(command, 'ar', { data: true }), { text: command, localized: false });
  const closed = localizeOwnedText(command, 'ar');
  assert.equal(closed.localized, true);
  assert.ok(!closed.text.includes('Vendor App'), closed.text);
  // Known product patterns still translate for a data caller.
  assert.match(localizeOwnedText('Service: wuauserv', 'ar', { data: true }).text, arabic);
});

// P77-02: the ids the diagnostics engine puts on screen - deep-scan collectors and provider
// faults - are named for the reader in both languages. An id nobody named reads as a generic
// source, never as itself. The ids are read from the Rust sources, so a new one fails here.
test('every deep-scan collector id the coordinator emits is named in both languages', async () => {
  const { readFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const coordinator = readFileSync(join(import.meta.dirname, '../../../crates/pc-intelligence/src/coordinator.rs'), 'utf8');
  const emitted = new Set([
    ...[...coordinator.matchAll(/\(\s*"([a-z]+)",\s*vec!\[/g)].map((m) => m[1]), // the two task batches
    ...[...coordinator.matchAll(/id: "([a-z]+)"\.into\(\)/g)].map((m) => m[1]), // updates, recovery
  ]);
  assert.deepEqual([...emitted].sort(), ['cleanup', 'diagnostics', 'drivers', 'recovery', 'startup', 'updates', 'windows']);
  for (const id of emitted) {
    const en = names.localizeScanCollector(id, 'en');
    const ar = names.localizeScanCollector(id, 'ar');
    assert.notEqual(en, id, id);
    assert.match(ar, arabic, id);
    assert.ok(!ar.includes(id), `${id} → ${ar}`);
  }
  const generic = names.localizeScanCollector('zzFutureCollector', 'ar');
  assert.match(generic, arabic);
  assert.ok(!generic.includes('zzFutureCollector'), generic);
});

test('every provider that reports a fault is named in both languages', async () => {
  const { readdirSync, readFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const providers = new Set<string>();
  for (const crate of ['hardware-telemetry', 'crash-diagnostics', 'diagnostic-engine', 'idle-scheduler']) {
    const root = join(import.meta.dirname, '../../../crates', crate, 'src');
    for (const file of readdirSync(root, { recursive: true }) as string[]) {
      if (!file.endsWith('.rs')) continue;
      const text = readFileSync(join(root, file), 'utf8');
      for (const m of text.matchAll(/CollectorFault::(?:new|cancelled|timeout)\(\s*"([a-z-]+)"/g)) providers.add(m[1]);
    }
  }
  for (const known of ['hardware-telemetry', 'crash-diagnostics', 'diagnostic-engine', 'idle-scheduler']) {
    assert.ok(providers.has(known), `${known} is no longer read from the sources: ${[...providers]}`);
  }
  for (const id of providers) {
    const ar = names.localizeFaultProvider(id, 'ar');
    assert.notEqual(names.localizeFaultProvider(id, 'en'), id, id);
    assert.match(ar, arabic, id);
    assert.ok(!ar.includes(id), `${id} → ${ar}`);
  }
  assert.ok(!names.localizeFaultProvider('zz-future-provider', 'ar').includes('zz-future-provider'));
});

test('the deep-scan and provider-fault views do not print collector or provider ids', async () => {
  const { readFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const page = readFileSync(join(import.meta.dirname, '../src/features/intelligence/DeepScanPage.svelte'), 'utf8');
  const panel = readFileSync(join(import.meta.dirname, '../src/features/diagnostics/ProviderFaultsPanel.svelte'), 'utf8');
  assert.doesNotMatch(page, /<TechnicalText value=\{collector\.id\}/, 'the deep scan prints a collector id as its heading');
  assert.doesNotMatch(panel, /<TechnicalText value=\{fault\.(provider|operation)\}/, 'the fault panel prints a provider or operation id');
});

// P77-03: what the leak gate cannot see in the fixture, these read from the Rust sources. A sentence a
// crate writes for the reader either has an Arabic rendering or the boundary replaces it with
// "Details unavailable" - and that turns real information into nothing, so it is a failure here.
test('every sentence the UI-feeding crates write for the reader has an Arabic rendering', async () => {
  const { readdirSync, readFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const crates = ['cleaner', 'startup-manager', 'system-repair', 'hardware-telemetry', 'crash-diagnostics', 'diagnostic-engine',
    'driver-hub', 'driver-install', 'driver-backup', 'windows-update', 'performance-telemetry', 'pc-intelligence', 'windows-pnp',
    'support-bundle', 'update-engine', 'operation-engine', 'idle-scheduler'];
  const sentences = new Set<string>();
  for (const crate of crates) {
    const root = join(import.meta.dirname, '../../../crates', crate, 'src');
    for (const file of readdirSync(root, { recursive: true }) as string[]) {
      if (!file.endsWith('.rs')) continue;
      let text = readFileSync(join(root, file), 'utf8');
      const tests = text.indexOf('#[cfg(test)]');
      if (tests > 0) text = text.slice(0, tests); // test-only strings never reach a screen
      for (const m of text.matchAll(/"([A-Z][a-z][^"\\{}\n]{18,}\.)"/g)) if (m[1].split(' ').length >= 4) sentences.add(m[1]);
    }
  }
  assert.ok(sentences.size > 80, `read only ${sentences.size} sentences; the extraction is broken`);
  const fallback = td('text.unavailable', 'ar');
  const missing = [...sentences].filter((sentence) => localizeOwnedText(sentence, 'ar').text === fallback).sort();
  assert.deepEqual(missing, [], `sentences with no Arabic rendering:\n  ${missing.join('\n  ')}`);
});

test('an identifier stays data, a bare word does not, and a dropped sentence is reported once', async () => {
  const fallback = td('text.unavailable', 'ar');
  assert.equal(localizeOwnedText('MSFT_StorageReliabilityCounter', 'ar').text, 'MSFT_StorageReliabilityCounter');
  assert.equal(localizeOwnedText('0x8024402C', 'ar').text, '0x8024402C');
  assert.equal(localizeOwnedText('Timeout', 'ar').text, fallback, 'a bare English word is prose, not an identifier');
  assert.equal(localizeOwnedText('Failed.', 'ar').text, fallback, 'a bare word with a full stop is prose');
  assert.equal(localizeOwnedText('Windows.Update.Client', 'ar').text, 'Windows.Update.Client', 'a dotted name is an identifier');
  const { mock } = await import('node:test');
  const warn = mock.method(console, 'warn', () => {});
  try {
    localizeOwnedText('A sentence nobody translated yet.', 'ar');
    localizeOwnedText('A sentence nobody translated yet.', 'ar');
    assert.equal(warn.mock.callCount(), 1, 'the local log names a dropped sentence once, not on every render');
    assert.match(String(warn.mock.calls[0].arguments[1]), /nobody translated/);
  } finally {
    warn.mock.restore();
  }
});
