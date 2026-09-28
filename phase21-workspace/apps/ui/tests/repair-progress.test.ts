// P76 DBT-P76-007: "Assessing…" for 10+ minutes on the owner's install. The page names the
// check running, for how long; every check id the Windows assessment reports has a label in
// both languages (the ids are read from crates/system-repair/src/windows_impl.rs).
// Run: node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { hasMessageKey, td } from '../src/lib/i18n/index.ts';

const arabic = /[؀-ۿ]/;

test('every assessment check the Windows provider reports has an English and an Arabic label', () => {
  const rust = readFileSync(new URL('../../../crates/system-repair/src/windows_impl.rs', import.meta.url), 'utf8');
  const assess = rust.slice(rust.indexOf('fn assess('), rust.indexOf('fn repair('));
  const ids = [...assess.matchAll(/step\(\s*&mut checks,\s*"([a-z-]+)"/g)].map((m) => m[1]);
  assert.ok(ids.includes('dism-scan') && ids.includes('sfc-verify') && ids.includes('disk-scan'), `${ids}`);
  for (const id of ids) {
    const key = `repair.check.${id}`;
    assert.ok(hasMessageKey(key), key);
    assert.match(td(key as never, 'ar'), arabic, key);
  }
});
