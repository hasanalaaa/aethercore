// P75 trial run: values the service emits that the UI printed raw (English inside the Arabic UI).
// Run: node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { localizeFactState, localizeMatchQuality, localizeRecommendationReason, localizeStartupScope, tp } from '../src/lib/i18n/index.ts';

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

test('insight summary keys the service sends are in the catalog', async () => {
  const { hasMessageKey } = await import('../src/lib/i18n/index.ts');
  // crates/intelligence-core: llama.rs (model insights) and engine.rs (rule engine)
  for (const key of ['insight.summary.observation', 'insight.summary.securityPosture', 'insight.summary.bottleneck', 'insight.summary.recurrence', 'insight.summary.repairState']) {
    assert.ok(hasMessageKey(key), key);
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

