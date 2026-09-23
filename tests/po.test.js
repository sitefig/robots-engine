// Translations: gettext .po files are the source, locales/*.json is
// generated from them. Checks the plural rules, the file format and that
// the generated dictionaries are up to date.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { PLURAL_FORMS, categories, pluralIndex, nplurals, parsePo, writePo, toDictionary, merge } from '../tools/po.ts';
import { outputs } from '../tools/i18n.ts';
import { LANGUAGES } from '../tools/languages.ts';

test('every language has Plural-Forms that pick the same CLDR category as Intl', () => {
  const ns = [...Array(2001).keys(), 1e6, 2e6, 1000001, 1100000];
  for (const code of Object.keys(LANGUAGES)) {
    const forms = PLURAL_FORMS[code];
    assert.ok(forms, `${code} has no Plural-Forms`);
    const cats = categories(code);
    assert.equal(nplurals(forms), cats.length, `${code} nplurals`);
    const rules = new Intl.PluralRules(code);
    for (const n of ns) assert.equal(cats[pluralIndex(forms, n)], rules.select(n), `${code} n=${n}`);
  }
});

test('every .po file parses, declares its language, and writes back unchanged', () => {
  for (const code of Object.keys(LANGUAGES)) {
    const text = readFileSync(new URL(`../po/${code}.po`, import.meta.url), 'utf8');
    const po = parsePo(text);
    assert.equal(po.headers.Language, code);
    assert.equal(po.headers['Plural-Forms'], PLURAL_FORMS[code]);
    assert.equal(writePo(po), text, `${code}.po is not in canonical form`);
  }
});

test('locales/*.json and po/messages.pot are generated from po/', () => {
  for (const [rel, content] of Object.entries(outputs())) {
    assert.equal(readFileSync(new URL(`../${rel}`, import.meta.url), 'utf8'), content, `${rel} is stale: run npm run i18n`);
  }
});

test('escapes, multi-line strings and plurals survive a round trip', () => {
  const po = {
    headers: { Language: 'cs', 'Plural-Forms': PLURAL_FORMS.cs },
    entries: [
      { key: 'a.quote', msgid: 'Say "hi"\\now', msgidPlural: null, msgstr: ['Řekni "ahoj"\\teď'], comments: [], flags: [] },
      { key: 'a.lines', msgid: 'one\ntwo', msgidPlural: null, msgstr: ['jedna\ndvě\n'], comments: ['note'], flags: [] },
      { key: 'a.n', msgid: '{n} file', msgidPlural: '{n} files', msgstr: ['{n} soubor', '{n} soubory', '{n} souboru', '{n} souborů'], comments: [], flags: [] },
    ],
  };
  const back = parsePo(writePo(po));
  assert.deepEqual(back.entries, po.entries);
  assert.deepEqual(toDictionary('cs', back)['a.n'], { one: '{n} soubor', few: '{n} soubory', many: '{n} souboru', other: '{n} souborů' });
});

test('merge keeps translations, adds new keys, and flags changed English as fuzzy', () => {
  const current = parsePo(writePo({
    headers: {},
    entries: [
      { key: 'k.same', msgid: 'Hello', msgidPlural: null, msgstr: ['Hallo'], comments: [], flags: [] },
      { key: 'k.changed', msgid: 'Old text', msgidPlural: null, msgstr: ['Alter Text'], comments: [], flags: [] },
      { key: 'k.gone', msgid: 'Removed', msgidPlural: null, msgstr: ['Entfernt'], comments: [], flags: [] },
    ],
  }));
  const merged = merge('de', 'Deutsch', { 'k.same': 'Hello', 'k.changed': 'New text', 'k.new': 'Fresh' }, current);
  const by = Object.fromEntries(merged.entries.map((e) => [e.key, e]));
  assert.deepEqual(Object.keys(by), ['k.same', 'k.changed', 'k.new']);
  assert.equal(by['k.same'].msgstr[0], 'Hallo');
  assert.deepEqual(by['k.changed'].flags, ['fuzzy']);
  assert.equal(by['k.new'].msgstr[0], '');
  const dict = toDictionary('de', merged);
  assert.equal(dict['k.same'], 'Hallo');
  assert.ok(!('k.changed' in dict), 'fuzzy entries fall back to English');
  assert.ok(!('k.new' in dict));
});
