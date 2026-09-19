#!/usr/bin/env node
// Translations: po/<code>.po is the source, locales/<code>.json is generated
// for the engine (Rust and WebAssembly), the CLI and the page.
//
//   node tools/i18n.ts build   po -> locales/*.json and po/messages.pot
//   node tools/i18n.ts sync    after editing po/en.po: add, drop or flag keys
//                              in every other .po, then build
//   node tools/i18n.ts check   exit 1 when locales/*.json is out of date
//
// English lives in po/en.po (msgstr is the English text). A new string is
// added there, then `npm run i18n:sync`.

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { LANGUAGES } from '../src/client/i18n.ts';
import { parsePo, writePo, toDictionary, englishFromPo, merge, headersFor, type Dictionary, type PoFile } from '../src/lib/po.ts';

const ROOT = new URL('../', import.meta.url);
const path = (rel: string) => fileURLToPath(new URL(rel, ROOT));
const readPo = (code: string): PoFile => parsePo(readFileSync(path(`po/${code}.po`), 'utf8'));
const json = (d: Dictionary) => JSON.stringify(d, null, 2) + '\n';

export function english(): Dictionary {
  return englishFromPo(readPo('en'));
}

/** Every generated file as { relative path: content }. */
export function outputs(): Record<string, string> {
  const en = english();
  // crates/core embeds its own copy so it can be published on crates.io.
  const out: Record<string, string> = { 'locales/en.json': json(en), 'crates/core/data/en.json': json(en) };
  for (const code of Object.keys(LANGUAGES)) {
    if (code === 'en') continue;
    out[`locales/${code}.json`] = json(toDictionary(code, readPo(code)));
  }
  const pot = readPo('en');
  pot.headers = { ...headersFor('en', 'sus.bot'), Language: '' };
  delete pot.headers['Language-Team'];
  pot.entries = pot.entries.map((e) => ({ ...e, msgstr: e.msgidPlural === null ? [''] : ['', ''], flags: [] }));
  out['po/messages.pot'] = writePo(pot);
  return out;
}

function build(): void {
  for (const [rel, content] of Object.entries(outputs())) {
    const file = path(rel);
    if (existsSync(file) && readFileSync(file, 'utf8') === content) continue;
    writeFileSync(file, content);
    console.log(`wrote ${rel}`);
  }
}

function sync(): void {
  const en = english();
  for (const [code, name] of Object.entries(LANGUAGES)) {
    if (code === 'en') continue;
    const merged = merge(code, name, en, readPo(code));
    const text = writePo(merged);
    if (text !== readFileSync(path(`po/${code}.po`), 'utf8')) {
      writeFileSync(path(`po/${code}.po`), text);
      const fuzzy = merged.entries.filter((e) => e.flags.includes('fuzzy')).length;
      console.log(`merged po/${code}.po${fuzzy ? ` (${fuzzy} fuzzy)` : ''}`);
    }
  }
  build();
}

function check(): number {
  const stale = Object.entries(outputs()).filter(([rel, content]) => !existsSync(path(rel)) || readFileSync(path(rel), 'utf8') !== content).map(([rel]) => rel);
  if (stale.length) console.error(`out of date, run npm run i18n: ${stale.join(', ')}`);
  return stale.length ? 1 : 0;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const cmd = process.argv[2] ?? 'build';
  if (cmd === 'build') build();
  else if (cmd === 'sync') sync();
  else if (cmd === 'check') process.exit(check());
  else {
    console.error(`unknown command ${cmd}; use build, sync or check`);
    process.exit(2);
  }
}
