import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { registerHooks } from 'node:module';
import { compile } from 'svelte/compiler';
import { render } from 'svelte/server';
import { td } from '../src/lib/i18n/index.ts';
import type { PcFinding, PcRemediationCandidate } from '../src/lib/contracts.ts';

// Compile the real card and its children: assertions cover source-to-render, not a duplicate view.
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
const { default: FindingCard } = await import('../src/features/intelligence/FindingCard.svelte');
const finding = {
  id: 'whea', code: 'HARDWARE_ERROR_EVIDENCE', domain: 2, severity: 3, confidence: 4,
  titleKey: 'finding.hardwareEvidence.title', summaryKey: 'finding.hardwareEvidence.summary', messageArgs: [],
  evidence: [{ factId: 'event', kind: 'HardwareEvent', source: 'WHEA', observedUnixMs: 1000, technicalValue: 'corrected=2' }],
  uncertaintyKey: 'finding.uncertainty.low', verificationStatus: 'ConfirmedCurrent',
  remediationAvailable: true, remediationSafety: 5, rebootRequirement: 'None', reversibility: 'Unknown',
  affectedResource: { kind: 'memory', stableId: 'm', displayName: 'Memory' }, correlation: null, ruleId: 'P17-HW-001', ruleVersion: 2,
} as PcFinding;
const candidate = { findingId: 'whea', descriptionKey: 'remediation.hardware_error_evidence' } as PcRemediationCandidate;
function card(locale: 'en' | 'ar', value = finding, candidates = [candidate]) {
  return render(FindingCard, { props: { locale, finding: value, candidates,
    evidence: { cite: 'WHEA · 1', raw: 'corrected=2' } } }).body;
}
for (const locale of ['en', 'ar'] as const) {
  test(`${locale}: corrected-error evidence shows measured scope, uncertainty, unmeasured cause and owned next action`, () => {
    const html = card(locale);
    for (const key of ['deepScan.findingScope', 'deepScan.findingLimits', 'deepScan.limits.hardware',
      'finding.uncertainty.low', 'deepScan.nextAction', 'remediation.hardware_error_evidence']) {
      assert.ok(html.includes(td(key as never, locale, { count: '1' })), key);
    }
    assert.ok(html.includes('WHEA · 1'), 'the existing evidence disclosure remains available');
  });
  test(`${locale}: missing recheck beats action; unknown candidate and missing timestamp remain honest`, () => {
    const recheck = card(locale, { ...finding, verificationStatus: 'VerificationUnavailable' });
    assert.ok(recheck.includes(td('deepScan.next.recheck' as never, locale)));
    assert.ok(!recheck.includes(td('remediation.hardware_error_evidence', locale)));
    const unknown = card(locale, { ...finding, code: 'UNKNOWN', uncertaintyKey: 'raw.unknown',
      evidence: [{ ...finding.evidence[0], observedUnixMs: 0 }] }, [{ ...candidate, descriptionKey: 'raw.unknown' }]);
    assert.ok(unknown.includes(td('deepScan.limits.observations' as never, locale)));
    assert.ok(unknown.includes(td('deepScan.next.review' as never, locale)));
    assert.ok(unknown.includes(td('deepScan.findingTimeUnknown' as never, locale)));
    assert.ok(!unknown.includes('raw.unknown'));
  });
}


for (const locale of ['en', 'ar'] as const) {
  test(`${locale}: real adapter window card keeps exact counts, time, source and owned limits`, () => {
    const args = { windowMs: '1000', inErrors: '18446744073709551615', outErrors: '0', inDiscards: '0', outDiscards: '0' };
    const value = { ...finding, code: 'NETWORK_COUNTER_ERRORS_OBSERVED', severity: 1, confidence: 5,
      titleKey: 'finding.networkWindow.title', summaryKey: 'finding.networkWindow.summary',
      messageArgs: Object.entries(args).map(([key, value]) => ({ key, value })),
      uncertaintyKey: 'finding.networkWindow.limits', remediationAvailable: false,
      evidence: [{ factId: 'window', kind: 'DeviceState', source: 'GetIfEntry2', observedUnixMs: 1000, technicalValue: 'windowMs=1000' }],
    } as PcFinding;
    const html = card(locale, value, []);
    assert.ok(html.includes(td('finding.networkWindow.summary' as never, locale, args)));
    assert.ok(html.includes(td('finding.networkWindow.limits' as never, locale)));
    assert.ok(html.includes(td('deepScan.severity.informational' as never, locale)));
    assert.ok(html.includes('18446744073709551615'));
    assert.ok(!html.includes('finding.networkWindow.'));
  });
}


for (const locale of ['en', 'ar'] as const) {
  test(`${locale}: absolute battery interpretation keeps measured capacities, estimate and owned limits`, () => {
    const args = { designCapacityMwh: '56000', fullChargeCapacityMwh: '41000', lossPercent: '26.7' };
    const value = { ...finding, code: 'BATTERY_CAPACITY_BELOW_DESIGN', severity: 1, confidence: 5,
      titleKey: 'finding.batteryCapacity.title', summaryKey: 'finding.batteryCapacity.summary',
      messageArgs: Object.entries(args).map(([key, value]) => ({ key, value })),
      uncertaintyKey: 'finding.batteryCapacity.limits', remediationAvailable: false,
      evidence: [{ factId: 'capacity', kind: 'DeviceState', source: 'IOCTL_BATTERY_QUERY_INFORMATION', observedUnixMs: 1000, technicalValue: 'designCapacityMwh=56000;fullChargeCapacityMwh=41000' }],
    } as PcFinding;
    const html = card(locale, value, []);
    assert.ok(html.includes(td('finding.batteryCapacity.summary' as never, locale, args)));
    assert.ok(html.includes(td('finding.batteryCapacity.limits' as never, locale)));
    assert.ok(html.includes(td('deepScan.severity.informational' as never, locale)));
    assert.ok(!html.includes('finding.batteryCapacity.'));
  });
}
