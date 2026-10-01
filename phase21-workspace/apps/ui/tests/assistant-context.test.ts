import { test } from 'node:test';
import assert from 'node:assert/strict';
import { knownContext } from '../src/features/assistant/context.ts';
test('context entry requires an exact current evidence id and surface', () => {
  const pack = [{ evidenceId: 'scan-a:disk-risk', surface: 'diagnostics', detail: '' }];
  assert.equal(knownContext(pack, 'scan-a:disk-risk', 'diagnostics'), true);
  assert.equal(knownContext(pack, 'scan-b:disk-risk', 'diagnostics'), false);
  assert.equal(knownContext(pack, 'scan-a:disk-risk', 'repairDiagnosis'), false);
  assert.equal(knownContext([], 'scan-a:disk-risk', 'diagnostics'), false);
});
