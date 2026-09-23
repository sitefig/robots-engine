// Consistency of the dictionaries: every locale is a subset of English with
// the same placeholders, and every key the source code asks for exists.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { LANGUAGES } from '../tools/languages.ts';

const LOCALE_DIR = new URL('../locales/', import.meta.url);
const codes = readdirSync(LOCALE_DIR).filter((f) => f.endsWith('.json')).map((f) => f.slice(0, -5));
const load = (code) => JSON.parse(readFileSync(new URL(`${code}.json`, LOCALE_DIR), 'utf8'));
const en = load('en');
const placeholders = (v) => new Set(String(v).match(/\{[a-zA-Z0-9_]+\}/g) || []);
const forms = (v) => (v && typeof v === 'object' ? Object.values(v) : [v]);

test('every locale file is a known language and English lists no unknown code', () => {
  for (const code of codes) assert.ok(code in LANGUAGES, `unknown locale file ${code}.json`);
  for (const code of Object.keys(LANGUAGES)) assert.ok(codes.includes(code), `missing locale file for ${code}`);
});

test('English has no empty values and every plural object has `other`', () => {
  for (const [k, v] of Object.entries(en)) {
    if (typeof v === 'object') assert.ok(typeof v.other === 'string' && v.other, `${k} needs other`);
    for (const f of forms(v)) assert.ok(typeof f === 'string' && f.trim(), `${k} is empty`);
  }
});

for (const code of codes.filter((c) => c !== 'en')) {
  test(`locale ${code} is a consistent subset of English`, async () => {
    const dict = load(code);
    for (const [k, v] of Object.entries(dict)) {
      assert.ok(Object.hasOwn(en, k), `${code}: unknown key ${k}`);
      const enForms = forms(en[k]);
      const enPlaceholders = new Set(enForms.flatMap((f) => [...placeholders(f)]));
      if (typeof v === 'object') assert.ok(typeof v.other === 'string' && v.other, `${code}: ${k} needs other`);
      for (const f of forms(v)) {
        assert.ok(typeof f === 'string' && f.trim(), `${code}: ${k} is empty`);
        for (const ph of placeholders(f)) assert.ok(enPlaceholders.has(ph), `${code}: ${k} uses unknown placeholder ${ph}`);
      }
      // Every placeholder English uses must appear in at least one form, except `{n}` in plurals where a form may spell the number out.
      for (const ph of enPlaceholders) {
        if (ph === '{n}' && typeof v === 'object') continue;
        assert.ok(forms(v).some((f) => f.includes(ph)), `${code}: ${k} drops placeholder ${ph}`);
      }
    }
  });
}

test('every dictionary key referenced in the source exists in English', () => {
  const walk = (dir, exts) => readdirSync(new URL(`../${dir}/`, import.meta.url), { withFileTypes: true }).flatMap((d) => {
    const rel = `${dir}/${d.name}`;
    if (d.isDirectory()) return d.name === 'wasm' || d.name === 'target' ? [] : walk(rel, exts);
    if (!exts.some((e) => d.name.endsWith(e))) return [];
    let text = readFileSync(new URL(`../${rel}`, import.meta.url), 'utf8');
    if (d.name.endsWith('.rs')) text = text.split('#[cfg(test)]')[0]; // unit tests use made-up keys
    return [[rel, text]];
  });
  const src = [...walk('tools', ['.ts']), ...walk('crates', ['.rs']), ...walk('config', ['.toml'])];
  const keys = Object.keys(en);
  const exists = (key) => keys.includes(key) || keys.some((k) => k.startsWith(`${key}.`));
  const hasPrefix = (prefix) => keys.some((k) => k.startsWith(prefix));
  const checked = new Set();
  for (const [name, text] of src) {
    for (const m of text.matchAll(/\b(?:t|tx|s)\(\s*["']([a-z]+(?:\.[A-Za-z0-9-]+)+)["']/g)) {
      checked.add(m[1]);
      assert.ok(exists(m[1]), `${name}: t('${m[1]}') has no dictionary entry`);
    }
    for (const m of text.matchAll(/\b(?:issue|warn|push|push_variant|w)\((?:[^()]*?),\s*["']([a-z]+(?:\.[A-Za-z0-9-]+)+)["']/g)) {
      checked.add(m[1]);
      assert.ok(exists(m[1]), `${name}: issue id '${m[1]}' has no dictionary entry`);
    }
    for (const m of text.matchAll(/by_?[iI]d\((?:&\[)?([^)\]]*)/g)) {
      for (const id of m[1].match(/["'][a-z]+\.[^"']+["']/g) || []) assert.ok(exists(id.slice(1, -1)), `${name}: byId(${id}) has no dictionary entry`);
    }
    for (const m of text.matchAll(/\bt\(\s*`([a-z]+(?:\.[A-Za-z0-9-]+)*\.)\$\{/g)) {
      assert.ok(hasPrefix(m[1]), `${name}: dynamic key prefix ${m[1]} matches nothing`);
    }
    for (const m of text.matchAll(/format!\("((?:enum|recon|agents|security|csv|md|ui|page|ai|parser|seo|sitemap|lint|fetch)(?:\.[A-Za-z0-9-]+)*\.)\{/g)) {
      assert.ok(hasPrefix(m[1]), `${name}: dynamic key prefix ${m[1]} matches nothing`);
    }
    for (const m of text.matchAll(/^\s*(?:label|advice|reason|note|tech|feed_fallback|portal_fallback) = "([a-z]+\.[A-Za-z0-9.]+)"/gm)) {
      assert.ok(exists(m[1]), `${name}: config key ${m[1]} has no dictionary entry`);
    }
    // page components: e('page.x'), raw('page.x'), text('page.x')
    for (const m of text.matchAll(/\b(?:e|raw|text)\(\s*'([a-z]+(?:\.[A-Za-z0-9-]+)+)'/g)) {
      checked.add(m[1]);
      assert.ok(exists(m[1]), `${name}: page key ${m[1]} has no dictionary entry`);
    }
  }
  assert.ok(checked.size > 100, 'the scan found the t() calls');
});
