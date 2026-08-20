'use strict';
const assert = require('node:assert/strict');

const ar = new Intl.PluralRules('ar');
const expected = new Map([[0,'zero'],[1,'one'],[2,'two'],[3,'few'],[7,'few'],[11,'many'],[99,'many'],[100,'other'],[102,'other']]);
for (const [value, category] of expected) assert.equal(ar.select(value), category, `Arabic plural category for ${value}`);
const en = new Intl.PluralRules('en');
assert.equal(en.select(1), 'one');
assert.equal(en.select(0), 'other');
assert.equal(en.select(2), 'other');

const arNumber = new Intl.NumberFormat('ar-IQ').format(1234567);
assert.notEqual(arNumber, '1,234,567', 'ar-IQ number formatting must be locale-sensitive');
const arDate = new Intl.DateTimeFormat('ar-IQ', { dateStyle: 'medium', timeStyle: 'short', timeZone: 'UTC' }).format(new Date('2026-08-19T12:34:00Z'));
assert.ok(arDate.length > 4);

console.log('Phase 12 Intl/pluralization tests: PASS');
