// Gettext .po files are the translation source; the engine and the page
// read locales/<code>.json, which tools/i18n.ts generates from them.
//
// Each message is keyed by its dictionary key in msgctxt; msgid holds the
// English text so translators see the source, msgstr the translation.
// Plurals use msgid_plural and msgstr[i]; index i is the i-th CLDR plural
// category of the language (canonical order zero, one, two, few, many,
// other), and PLURAL_FORMS gives each language a gettext expression that
// yields that same index. tests/po.test.js checks every expression
// against Intl.PluralRules.

export type Value = string | Record<string, string>;
export type Dictionary = Record<string, Value>;

export interface Entry {
  key: string;
  msgid: string;
  msgidPlural: string | null;
  msgstr: string[];
  comments: string[];
  /** Gettext flags such as "fuzzy" (a translation of an older English text). */
  flags: string[];
}

export interface PoFile {
  headers: Record<string, string>;
  entries: Entry[];
}

const CANONICAL = ['zero', 'one', 'two', 'few', 'many', 'other'];

/** CLDR plural categories of a language, in canonical order. */
export function categories(lang: string): string[] {
  const found = new Intl.PluralRules(lang).resolvedOptions().pluralCategories as string[];
  return CANONICAL.filter((c) => found.includes(c));
}

/**
 * Gettext Plural-Forms per language. Index i must select categories(lang)[i]
 * for every integer; categories that only apply to decimals keep their slot
 * and are never selected for whole numbers.
 */
export const PLURAL_FORMS: Record<string, string> = {
  bg: 'nplurals=2; plural=(n != 1);',
  cs: 'nplurals=4; plural=(n == 1 ? 0 : n >= 2 && n <= 4 ? 1 : 3);',
  da: 'nplurals=2; plural=(n != 1);',
  de: 'nplurals=2; plural=(n != 1);',
  el: 'nplurals=2; plural=(n != 1);',
  en: 'nplurals=2; plural=(n != 1);',
  es: 'nplurals=3; plural=(n == 1 ? 0 : n != 0 && n % 1000000 == 0 ? 1 : 2);',
  et: 'nplurals=2; plural=(n != 1);',
  fi: 'nplurals=2; plural=(n != 1);',
  fr: 'nplurals=3; plural=(n == 0 || n == 1 ? 0 : n != 0 && n % 1000000 == 0 ? 1 : 2);',
  ga: 'nplurals=5; plural=(n == 1 ? 0 : n == 2 ? 1 : n >= 3 && n <= 6 ? 2 : n >= 7 && n <= 10 ? 3 : 4);',
  hr: 'nplurals=3; plural=(n % 10 == 1 && n % 100 != 11 ? 0 : n % 10 >= 2 && n % 10 <= 4 && (n % 100 < 12 || n % 100 > 14) ? 1 : 2);',
  hu: 'nplurals=2; plural=(n != 1);',
  it: 'nplurals=3; plural=(n == 1 ? 0 : n != 0 && n % 1000000 == 0 ? 1 : 2);',
  lt: 'nplurals=4; plural=(n % 10 == 1 && (n % 100 < 11 || n % 100 > 19) ? 0 : n % 10 >= 2 && n % 10 <= 9 && (n % 100 < 11 || n % 100 > 19) ? 1 : 3);',
  lv: 'nplurals=3; plural=(n % 10 == 0 || (n % 100 >= 11 && n % 100 <= 19) ? 0 : n % 10 == 1 && n % 100 != 11 ? 1 : 2);',
  mt: 'nplurals=5; plural=(n == 1 ? 0 : n == 2 ? 1 : n == 0 || (n % 100 >= 3 && n % 100 <= 10) ? 2 : n % 100 >= 11 && n % 100 <= 19 ? 3 : 4);',
  nl: 'nplurals=2; plural=(n != 1);',
  pl: 'nplurals=4; plural=(n == 1 ? 0 : n % 10 >= 2 && n % 10 <= 4 && (n % 100 < 12 || n % 100 > 14) ? 1 : 2);',
  pt: 'nplurals=3; plural=(n == 0 || n == 1 ? 0 : n != 0 && n % 1000000 == 0 ? 1 : 2);',
  ro: 'nplurals=3; plural=(n == 1 ? 0 : n == 0 || (n % 100 >= 1 && n % 100 <= 19) ? 1 : 2);',
  sk: 'nplurals=4; plural=(n == 1 ? 0 : n >= 2 && n <= 4 ? 1 : 3);',
  sl: 'nplurals=4; plural=(n % 100 == 1 ? 0 : n % 100 == 2 ? 1 : n % 100 == 3 || n % 100 == 4 ? 2 : 3);',
  sv: 'nplurals=2; plural=(n != 1);',
};

/** Evaluates a Plural-Forms expression for n (only for tests and tools). */
export function pluralIndex(forms: string, n: number): number {
  const expr = /plural=(.*);?\s*$/.exec(forms)?.[1]?.replace(/;\s*$/, '');
  if (!expr) throw new Error(`no plural expression in "${forms}"`);
  if (!/^[\sn0-9()!=<>&|?:%+\-*/]+$/.test(expr)) throw new Error(`unexpected characters in "${expr}"`);
  return Number(new Function('n', `return (${expr});`)(n));
}

export function nplurals(forms: string): number {
  const m = /nplurals=(\d+)/.exec(forms);
  if (!m) throw new Error(`no nplurals in "${forms}"`);
  return Number(m[1]);
}

// ---------------------------------------------------------------- reading

function unquote(s: string): string {
  if (!s.startsWith('"') || !s.endsWith('"')) throw new Error(`not a quoted string: ${s}`);
  return s.slice(1, -1).replace(/\\(.)/g, (_, c: string) => ({ n: '\n', t: '\t', r: '\r', '"': '"', '\\': '\\' })[c] ?? c);
}

export function parsePo(text: string): PoFile {
  const entries: Entry[] = [];
  let headers: Record<string, string> = {};
  const blank = () => ({ ctx: null as string | null, id: null as string | null, plural: null as string | null, str: [] as string[], comments: [] as string[], flags: [] as string[] });
  let cur = blank();
  let field: { name: string; index: number } | null = null;

  const flush = () => {
    if (cur.id === null) {
      cur = blank();
      return;
    }
    if (cur.ctx === null && cur.id === '') {
      headers = Object.fromEntries(
        (cur.str[0] ?? '').split('\n').filter(Boolean).map((line) => {
          const i = line.indexOf(':');
          return [line.slice(0, i).trim(), line.slice(i + 1).trim()];
        }),
      );
    } else {
      if (cur.ctx === null) throw new Error(`entry without msgctxt: ${cur.id}`);
      entries.push({ key: cur.ctx, msgid: cur.id, msgidPlural: cur.plural, msgstr: cur.str, comments: cur.comments, flags: cur.flags });
    }
    cur = blank();
    field = null;
  };

  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (line === '') {
      flush();
      continue;
    }
    if (line.startsWith('#~')) continue; // obsolete entries are dropped
    if (line.startsWith('#')) {
      if (cur.id !== null) flush();
      if (line.startsWith('#.')) cur.comments.push(line.slice(2).trim());
      else if (line.startsWith('#,')) cur.flags.push(...line.slice(2).split(',').map((f) => f.trim()).filter(Boolean));
      continue;
    }
    let m: RegExpExecArray | null;
    if ((m = /^msgctxt\s+(".*")$/.exec(line))) {
      if (cur.id !== null) flush();
      cur.ctx = unquote(m[1]);
      field = { name: 'ctx', index: 0 };
    } else if ((m = /^msgid\s+(".*")$/.exec(line))) {
      cur.id = unquote(m[1]);
      field = { name: 'id', index: 0 };
    } else if ((m = /^msgid_plural\s+(".*")$/.exec(line))) {
      cur.plural = unquote(m[1]);
      field = { name: 'plural', index: 0 };
    } else if ((m = /^msgstr(?:\[(\d+)\])?\s+(".*")$/.exec(line))) {
      const index = m[1] === undefined ? 0 : Number(m[1]);
      cur.str[index] = unquote(m[2]);
      field = { name: 'str', index };
    } else if (line.startsWith('"') && field) {
      const s = unquote(line);
      if (field.name === 'ctx') cur.ctx = (cur.ctx ?? '') + s;
      else if (field.name === 'id') cur.id = (cur.id ?? '') + s;
      else if (field.name === 'plural') cur.plural = (cur.plural ?? '') + s;
      else cur.str[field.index] = (cur.str[field.index] ?? '') + s;
    } else {
      throw new Error(`cannot parse line: ${raw}`);
    }
  }
  flush();
  return { headers, entries };
}

// ---------------------------------------------------------------- writing

function quote(s: string): string {
  return `"${s.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\t/g, '\\t').replace(/\r/g, '\\r').replace(/\n/g, '\\n')}"`;
}

/** A keyword and its string, split after newlines like xgettext does. */
function field(keyword: string, value: string): string {
  if (!value.includes('\n') || value.indexOf('\n') === value.length - 1) return `${keyword} ${quote(value)}`;
  const parts = value.split(/(?<=\n)/);
  return [`${keyword} ""`, ...parts.map(quote)].join('\n');
}

export function writePo(file: PoFile): string {
  const header = Object.entries(file.headers).map(([k, v]) => `${k}: ${v}\n`).join('');
  const out = [`msgid ""\n${field('msgstr', header).replace(/^msgstr ""/, 'msgstr ""')}`];
  for (const e of file.entries) {
    const lines = e.comments.map((c) => `#. ${c}`);
    if (e.flags.length) lines.push(`#, ${e.flags.join(', ')}`);
    lines.push(field('msgctxt', e.key), field('msgid', e.msgid));
    if (e.msgidPlural !== null) {
      lines.push(field('msgid_plural', e.msgidPlural));
      e.msgstr.forEach((s, i) => lines.push(field(`msgstr[${i}]`, s ?? '')));
    } else {
      lines.push(field('msgstr', e.msgstr[0] ?? ''));
    }
    out.push(lines.join('\n'));
  }
  return out.join('\n\n') + '\n';
}

export function headersFor(lang: string, name: string): Record<string, string> {
  const forms = PLURAL_FORMS[lang];
  if (!forms) throw new Error(`no Plural-Forms for ${lang}`);
  return {
    'Project-Id-Version': 'sus.bot',
    'Language-Team': name,
    Language: lang,
    'MIME-Version': '1.0',
    'Content-Type': 'text/plain; charset=UTF-8',
    'Content-Transfer-Encoding': '8bit',
    'Plural-Forms': forms,
  };
}

// ---------------------------------------------------------------- conversion

/**
 * The .po file of `lang` for the English source `en`: every English key in
 * English order, with this language's translation or an empty msgstr.
 */
export function toPo(lang: string, name: string, en: Dictionary, dict: Dictionary): PoFile {
  const cats = categories(lang);
  const entries: Entry[] = Object.entries(en).map(([key, source]) => {
    const value = lang === 'en' ? source : dict[key];
    if (typeof source === 'string') {
      if (value !== undefined && typeof value !== 'string') throw new Error(`${lang}: ${key} is plural here but not in English`);
      return { key, msgid: source, msgidPlural: null, msgstr: [value ?? ''], comments: [], flags: [] };
    }
    const msgid = source.one ?? source.other;
    const msgstr = cats.map((c) => (value === undefined ? '' : typeof value === 'string' ? value : (value[c] ?? value.other ?? '')));
    return { key, msgid, msgidPlural: source.other, msgstr, comments: [], flags: [] };
  });
  return { headers: headersFor(lang, name), entries };
}

/** The dictionary a .po file describes; untranslated entries are left out. */
export function toDictionary(lang: string, po: PoFile): Dictionary {
  const cats = categories(lang);
  const forms = po.headers['Plural-Forms'];
  if (forms && nplurals(forms) !== cats.length) throw new Error(`${lang}: Plural-Forms has ${nplurals(forms)} forms, CLDR has ${cats.length}`);
  const out: Dictionary = {};
  for (const e of po.entries) {
    if (e.flags.includes('fuzzy')) continue; // needs review: English falls back
    if (e.msgidPlural === null) {
      if (e.msgstr[0]) out[e.key] = e.msgstr[0];
      continue;
    }
    if (!e.msgstr.some(Boolean)) continue;
    const obj: Record<string, string> = {};
    cats.forEach((c, i) => {
      const s = e.msgstr[i] || e.msgstr[cats.length - 1];
      if (s) obj[c] = s;
    });
    out[e.key] = obj;
  }
  return out;
}

/**
 * Brings a language's .po file in line with the English source: new keys
 * are added untranslated, removed keys dropped, and a translation whose
 * English text changed is kept but flagged fuzzy until someone reviews it.
 */
export function merge(lang: string, name: string, en: Dictionary, current: PoFile): PoFile {
  const old = new Map(current.entries.map((e) => [e.key, e]));
  const fresh = toPo(lang, name, en, {});
  for (const e of fresh.entries) {
    const prev = old.get(e.key);
    if (!prev || !prev.msgstr.some(Boolean)) continue;
    const plural = e.msgidPlural !== null;
    if (plural !== (prev.msgidPlural !== null)) continue; // shape changed: retranslate
    e.msgstr = plural ? e.msgstr.map((_, i) => prev.msgstr[i] ?? '') : [prev.msgstr[0] ?? ''];
    e.comments = prev.comments;
    const changed = prev.msgid !== e.msgid || (prev.msgidPlural ?? null) !== (e.msgidPlural ?? null);
    e.flags = changed || prev.flags.includes('fuzzy') ? ['fuzzy'] : prev.flags.filter((f) => f !== 'fuzzy');
  }
  return { headers: headersFor(lang, name), entries: fresh.entries };
}

/** English is its own source: msgstr equals the English text. */
export function englishFromPo(po: PoFile): Dictionary {
  const out: Dictionary = {};
  for (const e of po.entries) {
    out[e.key] = e.msgidPlural === null ? (e.msgstr[0] || e.msgid) : { one: e.msgstr[0] || e.msgid, other: e.msgstr[1] || e.msgidPlural };
  }
  return out;
}
