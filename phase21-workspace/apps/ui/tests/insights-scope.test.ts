import { test } from 'node:test';
import assert from 'node:assert/strict';
import { RequestScope } from '../src/features/insights/request-scope.ts';
test('a locale/evidence replacement rejects all outstanding replies', () => {
  const scope = new RequestScope();
  const first = scope.begin();
  assert.equal(scope.current(first), true);
  scope.invalidate();
  assert.equal(scope.current(first), false);
  const current = scope.begin();
  assert.equal(scope.current(current), true);
  scope.invalidate();
  assert.equal(scope.current(current), false);
});
