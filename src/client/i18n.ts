// Page-side translation for the strings the browser renders itself (ui.*,
// page.*, fetch.error.*). The analysis text is formatted inside the engine
// from the same dictionaries (locales/<code>.json, generated from po/); this
// module only needs the small subset the DOM uses, but loads the whole file
// for simplicity.
//
// t(key, params): current locale, then English, then the key itself.
// A value may be a plural object keyed by Intl.PluralRules category; the
// reserved `n` parameter selects the form.

export type Value = string | Record<string, string>;
export type Dictionary = Record<string, Value>;
export type Params = Record<string, string | number>;

export const DEFAULT_LANG = 'en';

// Official EU languages with their native names, in switcher order. Data only.
export const LANGUAGES: Record<string, string> = {
  bg: 'български', cs: 'čeština', da: 'dansk', de: 'Deutsch', el: 'Ελληνικά', en: 'English', es: 'español', et: 'eesti',
  fi: 'suomi', fr: 'français', ga: 'Gaeilge', hr: 'hrvatski', hu: 'magyar', it: 'italiano', lt: 'lietuvių', lv: 'latviešu',
  mt: 'Malti', nl: 'Nederlands', pl: 'polski', pt: 'português', ro: 'română', sk: 'slovenčina', sl: 'slovenščina', sv: 'svenska',
};

let lang = DEFAULT_LANG;
let english: Dictionary = {};
let dict: Dictionary = {};
let plurals = new Intl.PluralRules(DEFAULT_LANG);

/** The English dictionary must be set once before any t() call. */
export function setEnglish(strings: Dictionary): void {
  english = strings;
  if (lang === DEFAULT_LANG) dict = strings;
}

export function setLocale(code: string = DEFAULT_LANG, strings: Dictionary = {}): void {
  lang = code || DEFAULT_LANG;
  dict = lang === DEFAULT_LANG ? english : strings;
  try {
    plurals = new Intl.PluralRules(lang);
  } catch {
    plurals = new Intl.PluralRules(DEFAULT_LANG);
  }
}

export function getLocale(): string {
  return lang;
}

/** The current locale's dictionary, for the engine. */
export function currentDictionary(): Dictionary {
  return dict;
}

function pick(value: Value | undefined, n: string | number | undefined): string | undefined {
  if (value === null || typeof value !== 'object') return value;
  const category = typeof n === 'number' ? plurals.select(n) : 'other';
  return value[category] ?? value.other;
}

const PLACEHOLDER = /\{([a-zA-Z0-9_]+)\}/g;

export function t(key: string, params: Params = {}): string {
  let template = pick(dict[key], params.n);
  if (template === undefined) template = pick(english[key], params.n);
  if (template === undefined) return key;
  return String(template).replace(PLACEHOLDER, (m, name: string) => (Object.hasOwn(params, name) ? String(params[name]) : m));
}

export function formatNumber(n: number, options?: Intl.NumberFormatOptions): string {
  return new Intl.NumberFormat(lang, options).format(n);
}
