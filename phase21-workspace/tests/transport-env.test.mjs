import assert from 'node:assert/strict';
import test from 'node:test';
import { hasTauriInternals } from '../apps/ui/src/platform/transport-env.ts';

test('detects Tauri only when the runtime exposes its internals', () => {
  assert.equal(hasTauriInternals({}), false);
  assert.equal(hasTauriInternals({ __TAURI_INTERNALS__: undefined }), false);
  assert.equal(hasTauriInternals({ __TAURI_INTERNALS__: {} }), true);
});
