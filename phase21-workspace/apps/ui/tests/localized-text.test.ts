// P76, the owner's Windows 11 install at be965a2: backend ids and sentences the UI printed raw.
// The ids are read from the Rust sources that emit them, so a new one without a label fails here.
// Run: node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  localizeCleanupKind,
  localizeCleanupProvider,
  localizeCollector,
  localizeCollectorFault,
  localizeOwnedText,
} from '../src/lib/i18n/index.ts';

const arabic = /[؀-ۿ]/;
const workspace = new URL('../../../', import.meta.url);
const source = (path: string) => readFileSync(new URL(path, workspace), 'utf8');
const matches = (text: string, pattern: RegExp) => [...text.matchAll(pattern)].map((m) => m[1]);

test('DBT-P76-003: the disk health summary and its unreported counters are Arabic in the Arabic UI', () => {
  const summary = localizeOwnedText(
    'Windows reports this disk healthy; 2 SMART/reliability counter(s) were not reported and could not be independently checked.',
    'ar',
  );
  assert.ok(summary.localized);
  assert.match(summary.text, arabic);
  assert.match(summary.text, /2/);
  assert.doesNotMatch(summary.text, /Windows reports/);
  const rust = source('crates/hardware-telemetry/src/lib.rs');
  const names = matches(rust, /unavailable\.push\("([^"]+)"\)/g);
  assert.ok(names.length >= 4, `${names}`);
  for (const name of names) {
    const reason = localizeOwnedText(`${name}: not reported`, 'ar');
    assert.ok(reason.localized, name);
    assert.match(reason.text, arabic, name);
    assert.doesNotMatch(reason.text, /not reported/, name);
  }
});

test('DBT-P76-004: every performance collector id and fault state has an English and an Arabic label', () => {
  const rust = ['lib.rs', 'windows_impl.rs', 'macos_impl.rs', 'linux_impl.rs']
    .map((file) => source(`crates/performance-telemetry/src/${file}`))
    .join('\n');
  const ids = new Set(matches(rust, /collector: "([A-Za-z.]+)"/g));
  // Literal kinds, plus FaultKind's Debug names (lib.rs formats `{:?}` into the wire).
  const runtime = source('crates/collector-runtime/src/lib.rs');
  const faultKind = runtime.slice(runtime.indexOf('pub enum FaultKind'), runtime.indexOf('}', runtime.indexOf('pub enum FaultKind')));
  const kinds = new Set([...matches(rust, /kind: "([A-Za-z]+)"/g), ...matches(faultKind, /^\s+([A-Z][A-Za-z]+),$/gm)]);
  assert.ok(ids.has('processTop') && ids.has('power.temperature') && ids.has('memory.counters'), `${[...ids]}`);
  assert.ok(kinds.has('NotCollected') && kinds.has('Degraded') && kinds.has('MalformedResponse'), `${[...kinds]}`);
  for (const id of ids) {
    assert.match(localizeCollector(id, 'ar'), arabic, id);
    assert.notEqual(localizeCollector(id, 'en'), id, id);
  }
  for (const kind of kinds) {
    assert.match(localizeCollectorFault(kind, 'ar'), arabic, kind);
    assert.notEqual(localizeCollectorFault(kind, 'en'), kind, kind);
  }
});

test('DBT-P76-005: cleanup category ids and kinds are named, not printed, in both languages', () => {
  const rust = source('crates/cleaner/src/windows_impl.rs');
  const providers = new Set([
    ...matches(rust, /push_root\(\s*&mut output,\s*"([A-Za-z]+)"/g),
    ...matches(rust, /candidate\(\s*"([A-Za-z]+)"/g),
  ]);
  assert.deepEqual([...providers].sort(), ['ShaderCache', 'UserTemp', 'WindowsTemp']);
  // Historical plans still need names after P85 retains crash evidence instead of deleting it.
  providers.add('WER'); providers.add('CrashDumps');
  for (const id of providers) {
    assert.match(localizeCleanupProvider(id, 'ar'), arabic, id);
    assert.notEqual(localizeCleanupProvider(id, 'en'), id, id);
    const done = localizeOwnedText(`${id} provider completed`, 'ar');
    assert.match(done.text, arabic, id);
    assert.doesNotMatch(done.text, new RegExp(`\\b${id}\\b`), `the id itself is still in: ${done.text}`);
  }
  assert.match(localizeCleanupKind('Files', 'ar'), arabic);
  assert.notEqual(localizeCleanupKind('Files', 'en'), 'Files');
});
