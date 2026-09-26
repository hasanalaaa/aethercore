// P75 ui-truth. Run: node --experimental-strip-types --test tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { navigationShortcut } from '../src/lib/navigation.ts';

const press = (key: string, code: string, extra: Partial<KeyboardEvent> = {}) =>
  ({ ctrlKey: true, shiftKey: true, altKey: false, metaKey: false, key, code, ...extra });

test('Ctrl+Shift+1 on a US layout reaches Overview (Shift turns the key into "!")', () => {
  assert.equal(navigationShortcut(press('!', 'Digit1')), 'overview');
});

test('Ctrl+Shift+0 on a US layout reaches Activity (key ")")', () => {
  assert.equal(navigationShortcut(press(')', 'Digit0')), 'activity');
});

test('Ctrl+Shift+F on an Arabic layout reaches Fleet (key "ب")', () => {
  assert.equal(navigationShortcut(press('ب', 'KeyF')), 'fleet');
});

test('AltGr (Ctrl+Alt) combinations are not navigation', () => {
  assert.equal(navigationShortcut(press('!', 'Digit1', { altKey: true })), undefined);
});

test('without Shift nothing navigates', () => {
  assert.equal(navigationShortcut(press('1', 'Digit1', { shiftKey: false })), undefined);
});

import { commandShortcut } from '../src/lib/navigation.ts';

const chord = (key: string, code: string) =>
  ({ ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, key, code });

test('Ctrl+K on an Arabic layout opens the palette (key "ن")', () => {
  assert.equal(commandShortcut(chord('ن', 'KeyK')), 'palette');
});

test('Ctrl+/ on an Arabic layout opens the assistant (key "ظ")', () => {
  assert.equal(commandShortcut(chord('ظ', 'Slash')), 'assistant');
});

test('Ctrl+K on a US layout still opens the palette', () => {
  assert.equal(commandShortcut(chord('k', 'KeyK')), 'palette');
});
