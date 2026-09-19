import { test, after } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { t, setLocale, setEnglish, getLocale, formatNumber, LANGUAGES, DEFAULT_LANG } from '../src/client/i18n.ts';

const en = JSON.parse(readFileSync(new URL('../locales/en.json', import.meta.url), 'utf8'));
setEnglish(en);

after(() => setLocale());

test('English is the default and keys resolve with placeholders', () => {
  assert.equal(getLocale(), DEFAULT_LANG);
  assert.equal(t('parser.notFieldValue'), en['parser.notFieldValue']);
  assert.equal(t('parser.unknownDirective', { field: 'foo' }), 'Unknown directive "foo" is ignored.');
});

test('unknown keys come back as the key, unknown placeholders stay', () => {
  assert.equal(t('nope.missing'), 'nope.missing');
  setLocale('de', { 'x.y': 'Hallo {name}, {other}' });
  assert.equal(t('x.y', { name: 'Welt' }), 'Hallo Welt, {other}');
  setLocale();
});

test('placeholder values are not re-scanned for placeholders', () => {
  setLocale('de', { 'x.y': '{a} and {b}' });
  assert.equal(t('x.y', { a: '{b}', b: 'B' }), '{b} and B');
  setLocale();
});

test('plural forms follow Intl.PluralRules for the locale', () => {
  setLocale('cs', { 'x.n': { one: '{n} skupina', few: '{n} skupiny', other: '{n} skupin' } });
  assert.equal(t('x.n', { n: 1 }), '1 skupina');
  assert.equal(t('x.n', { n: 3 }), '3 skupiny');
  assert.equal(t('x.n', { n: 7 }), '7 skupin');
  // missing category falls back to `other`
  setLocale('cs', { 'x.n': { other: '{n} skupin' } });
  assert.equal(t('x.n', { n: 1 }), '1 skupin');
  setLocale();
  assert.equal(t('md.groups', { n: 1 }), '1 group');
  assert.equal(t('md.groups', { n: 2 }), '2 groups');
});

test('locale falls back to English key by key', () => {
  setLocale('de', { 'parser.notFieldValue': 'Zeile ignoriert.' });
  assert.equal(getLocale(), 'de');
  assert.equal(t('parser.notFieldValue'), 'Zeile ignoriert.');
  assert.equal(t('parser.bom'), en['parser.bom']);
  setLocale();
  assert.equal(t('parser.notFieldValue'), en['parser.notFieldValue']);
});

test('formatNumber uses the locale', () => {
  assert.equal(formatNumber(1234.5, { maximumFractionDigits: 1 }), '1,234.5');
  setLocale('de', {});
  assert.equal(formatNumber(1234.5, { maximumFractionDigits: 1 }), '1.234,5');
  setLocale();
});

test('language list covers the 24 official EU languages', () => {
  assert.equal(Object.keys(LANGUAGES).length, 24);
  for (const code of ['bg', 'cs', 'da', 'de', 'el', 'en', 'es', 'et', 'fi', 'fr', 'ga', 'hr', 'hu', 'it', 'lt', 'lv', 'mt', 'nl', 'pl', 'pt', 'ro', 'sk', 'sl', 'sv']) {
    assert.ok(code in LANGUAGES, code);
  }
});
