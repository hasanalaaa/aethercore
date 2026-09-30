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
  assert.equal(deepScanHeadlineKey(1, 3), 'deepScan.status.noneFound');
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

// P80-01: the Hardware verdict says what was measured and never "the whole PC is fine". Idle,
// nothing-measured, partial, attention, action and denied are different verdicts.
test('hardware verdict: not-collected, unavailable, partial, action and clear are distinct', async () => {
  const { hardwareVerdict } = await import('../src/features/intelligence/headline.ts');
  const disk = (severity: string) => ({ severity }) as never;
  const snap = (over: object) => ({ state: 'Ready', storage: [], memory: null, providerFaults: [], warnings: [], ...over }) as never;
  const memory = {} as never;
  assert.equal(hardwareVerdict(snap({ state: 'Idle' })).kind, 'notCollected');
  assert.equal(hardwareVerdict(snap({ state: 'Collecting' })).kind, 'collecting');
  const none = hardwareVerdict(snap({ providerFaults: [{ kindCode: 3 }] }));
  assert.equal(none.kind, 'unavailable');
  assert.equal(none.coverage, 'none');
  const partial = hardwareVerdict(snap({ storage: [disk('Normal')], memory: null }));
  assert.equal(partial.kind, 'noneFound');
  assert.equal(partial.coverage, 'partial', 'memory was not measured, so coverage is not complete');
  assert.deepEqual(partial.notMeasured, ['hardware.memoryLoad']);
  const faulted = hardwareVerdict(snap({ storage: [disk('Normal')], memory, providerFaults: [{ kindCode: 6 }] }));
  assert.equal(faulted.coverage, 'partial', 'a provider fault means a source did not answer');
  const clear = hardwareVerdict(snap({ storage: [disk('Normal')], memory }));
  assert.equal(clear.kind, 'noneFound');
  assert.equal(clear.coverage, 'complete');
  assert.equal(hardwareVerdict(snap({ storage: [disk('Attention')], memory })).kind, 'attention');
  assert.equal(hardwareVerdict(snap({ storage: [disk('ActionRequired')], memory })).kind, 'action');
  assert.equal(hardwareVerdict(snap({ storage: [disk('Normal')], memory, providerFaults: [{ kindCode: 4 }] })).denied, true, 'a denial is not damage');
  assert.equal(hardwareVerdict(snap({ storage: [disk('Normal')], memory })).denied, false);
});

// P81-04: three disks stay three verdicts. What one disk did not report is said for that disk
// only, by its own bus; the first advice for a data-risk disk is a backup, never a repair.
test('each disk states what it did not report, by its bus, and the first advice is a backup', async () => {
  const { missingDiskMetrics, diskAdvice } = await import('../src/features/diagnostics/disk-facts.ts');
  const has = { hasTemperature: true, hasWear: true, hasPowerOnHours: true, hasReadErrorsUncorrected: true, hasWriteErrorsUncorrected: true, hasNvmeCriticalWarning: true, hasNvmeAvailableSpare: true };
  const critical = { busType: 'NVMe', severity: 'ActionRequired', reliability: { ...has, wearPercentUsed: 105 } } as never;
  const usb = { busType: 'USB', severity: 'Unknown', reliability: null } as never;
  const sata = { busType: 'SATA', severity: 'Normal', reliability: { ...has, hasNvmeCriticalWarning: false, hasNvmeAvailableSpare: false } } as never;
  assert.deepEqual(missingDiskMetrics(critical), [], 'everything an NVMe disk reports was reported');
  assert.deepEqual(missingDiskMetrics(sata), [], 'a SATA disk is not charged with NVMe-only fields');
  const missing = missingDiskMetrics(usb);
  assert.ok(missing.includes('hardware.temperature') && missing.includes('hardware.wear'), 'a disk that reported nothing lists what is missing');
  assert.ok(!missing.includes('hardware.nvmeSpare'), 'and only what applies to its bus');
  assert.deepEqual(missingDiskMetrics({ busType: 'NVMe', severity: 'Normal', reliability: { ...has, hasTemperature: false, hasNvmeAvailableSpare: false } } as never), ['hardware.temperature', 'hardware.nvmeSpare']);
  assert.equal(diskAdvice(critical), 'hardware.disk.backupFirst');
  assert.equal(diskAdvice(usb), 'hardware.disk.unsupported', 'an unknown disk is unknown, not fine');
  assert.equal(diskAdvice(sata), null);
  assert.equal((critical as { reliability: { wearPercentUsed: number } }).reliability.wearPercentUsed, 105, 'wear over 100 is kept');
});

// P80-02B: presence survives to the screen. A sensor that read 0 shows 0; one that read nothing
// shows no value and says why (denied is not empty, unknown is not measured); bad input is skipped.
test('measurement rows keep "not read" apart from zero and survive malformed input', async () => {
  const { thermalRows, batteryRows, networkRows, bootRows } = await import('../src/features/diagnostics/measurement-rows.ts');
  const cov = (availability: number, extra: object = {}) => ({ source: 'ACPI', hasObservedUnixMs: true, observedUnixMs: 1_700_000_000_000, availability, ...extra });
  const zero = thermalRows([{ stableId: 'a', displayName: 'CPU', hasTemperature: true, temperatureC: 0, coverage: cov(1) }], 'en');
  const absent = thermalRows([{ stableId: 'b', displayName: 'GPU', hasTemperature: false, temperatureC: 0, coverage: cov(4) }], 'en');
  assert.equal(zero[0].value, '0 °C');
  assert.equal(absent[0].value, null);
  assert.equal(absent[0].availability, 'measurement.availability.denied');
  assert.equal(thermalRows([{ stableId: 'c', hasTemperature: false, coverage: cov(99) }], 'en')[0].availability, 'measurement.availability.unknown');
  assert.equal(thermalRows([{ stableId: 'd', hasTemperature: false, coverage: cov(3) }], 'en')[0].availability, 'measurement.availability.unsupported');
  assert.deepEqual(thermalRows(undefined as never, 'en'), []);
  assert.equal(thermalRows([null, 5, {}] as never, 'en').length, 1, 'only the object becomes a row, and it does not throw');
  const battery = batteryRows([{ stableId: 'bat', displayName: 'Battery', hasFullChargeCapacity: true, fullChargeCapacityMwh: 41000, hasDesignCapacity: true, designCapacityMwh: 50000, hasCycleCount: false, cycleCount: 0, coverage: cov(1) }], 'en');
  assert.match(battery[0].value ?? '', /41,000/);
  assert.match(battery[0].note ?? '', /50,000/, 'the design capacity is shown beside the current one, not instead of it');
  assert.doesNotMatch(battery[0].note ?? '', /cycle/i, 'cycle count was not read, so it is not stated as 0');
  assert.equal(networkRows([{ stableId: 'n', hasLinkSpeed: true, linkSpeedBps: 1_000_000_000, coverage: cov(1) }], 'en')[0].value, '1,000 Mbit/s');
  assert.equal(bootRows([{ recordedUnixMs: 1_700_000_000_000, hasDuration: false, durationMs: 0, coverage: cov(2) }], 'en')[0].value, null);
  const many = thermalRows(Array.from({ length: 32 }, (_, i) => ({ stableId: `z${i}`, hasTemperature: true, temperatureC: i, coverage: cov(1) })), 'ar');
  assert.equal(many.length, 32);
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

// P77-04A: the service's errors reach the reader as keys the catalogs can say, and a transport
// failure is classified instead of pasted. The keys are read from the service's Rust sources, so
// a key added there without a label fails here.
test('every message key the service emits resolves in both catalogs', async () => {
  const { readdirSync, readFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const { enCatalog } = await import('../src/lib/i18n/catalog.en.ts');
  const { arCatalog } = await import('../src/lib/i18n/catalog.ar.ts');
  const root = join(import.meta.dirname, '../../../services/maintenance-service/src');
  const shape = /"([a-z][A-Za-z0-9]*(?:\.[A-Za-z0-9_-]+)+)"/g;
  const emitted = new Set<string>();
  for (const file of readdirSync(root, { recursive: true }) as string[]) {
    if (!file.endsWith('.rs')) continue;
    const text = readFileSync(join(root, file), 'utf8');
    for (const call of text.matchAll(/ServiceError::[a-z_]+\(([^;]*?)\)/gs)) {
      for (const key of call[1].matchAll(shape)) if (!/\.(rs|json|toml|proto)$/.test(key[1])) emitted.add(key[1]);
    }
    if (file.endsWith('errors.rs')) for (const key of text.matchAll(shape)) if (!/\.(rs|json|toml|proto)$/.test(key[1])) emitted.add(key[1]);
  }
  assert.ok(emitted.size > 60, `read only ${emitted.size} keys; the extraction is broken`);
  const missing = [...emitted].filter((key) => !(key in enCatalog && key in arCatalog)).sort();
  assert.deepEqual(missing, [], `the service emits keys neither catalog can say: ${missing.join(', ')}`);
});

test('a transport failure is classified, a known key is kept, and raw text is never the answer', () => {
  const key = names.serviceErrorKey;
  // What the pages test with .includes() must survive.
  assert.equal(key('care.error.planChanged'), 'care.error.planChanged');
  assert.equal(key('Error: insight.error.modelLoading'), 'insight.error.modelLoading');
  // Transport and OS failures read as a class, not as the OS's sentence.
  assert.equal(key('connect to maintenance service: The system cannot find the file specified. (os error 2)'), 'service.error.unreachable');
  assert.equal(key('request timed out after 30s'), 'service.error.timeout');
  assert.equal(key('Access is denied. (os error 5)'), 'service.error.denied');
  // Anything else is a generic failure; the raw text stays in the local log, not on screen.
  assert.equal(key('Backend exploded'), 'service.error.failed');
  assert.equal(key(''), 'service.error.failed');
  for (const classified of ['service.error.unreachable', 'service.error.timeout', 'service.error.denied', 'service.error.failed']) {
    assert.equal(names.hasMessageKey(classified), true, classified);
    assert.match(names.td(classified as never, 'ar'), arabic, classified);
    assert.notEqual(names.td(classified as never, 'en'), names.td(classified as never, 'ar'), classified);
  }
});

// P78-01: the Windows Update health probe now answers from the local cache and says so. Its detail is
// composed with format!, which the Rust scan above cannot see, so the sentences are read out of the
// source here: a reworded sentence that loses its Arabic rendering fails on the next run.
test('the update health probe sentences read in Arabic', async () => {
  const { readFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const source = readFileSync(join(import.meta.dirname, '../../../crates/windows-update/src/windows_impl.rs'), 'utf8');
  const empty = 'Windows Update Agent answered from its local cache, which lists no pending updates; this does not show that Windows is up to date.';
  const counted = (n: number) => `Windows Update Agent answered from its local cache: ${n} pending update(s) are known locally; this is not a check for newer updates.`;
  assert.ok(source.includes(empty), 'the source no longer says the empty-cache sentence this test pins');
  assert.ok(source.includes(counted(0).replace('0', '{count}')), 'the source no longer says the counted sentence this test pins');
  const fallback = td('text.unavailable', 'ar');
  for (const sentence of [empty, counted(1), counted(12)]) {
    const shown = localizeOwnedText(sentence, 'ar').text;
    assert.notEqual(shown, fallback, sentence);
    assert.match(shown, arabic, sentence);
    assert.ok(!shown.includes('cache'), shown);
  }
  assert.match(localizeOwnedText(counted(12), 'ar').text, /12/);
});

// P78-03: a repair the owner cancels before the mutation barrier fails with this message. It is a
// lowercase `#[error]` string, which the sentence scan above does not read, so it is pinned here.
test('the message of a repair cancelled before any change reads in Arabic', async () => {
  const { readFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const source = readFileSync(join(import.meta.dirname, '../../../crates/system-repair/src/lib.rs'), 'utf8');
  const message = 'the repair was cancelled before any change was made';
  assert.ok(source.includes(`#[error("${message}")]`), 'the source no longer says the message this test pins');
  const shown = localizeOwnedText(message, 'ar').text;
  assert.notEqual(shown, td('text.unavailable', 'ar'));
  assert.match(shown, arabic);
});
