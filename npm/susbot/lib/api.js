// The JavaScript API over the WebAssembly engine. The wasm-bindgen glue
// passes JSON strings (as the site does); this layer takes an options object,
// parses the JSON and keeps the names of the Python package. node.js and
// web.js re-export it after (node) or around (web) initialising the module.

import * as glue from '../wasm/susbot_wasm.js';

/** The engine object behind each Analysis, for diff(). */
const inner = new WeakMap();

function optionsJson(options = {}) {
  const out = {};
  if (options.siteUrl != null) out.siteUrl = String(options.siteUrl);
  if (options.config != null) out.config = String(options.config);
  if (options.lang != null) out.lang = String(options.lang);
  if (options.locale != null) out.locale = typeof options.locale === 'string' ? options.locale : JSON.stringify(options.locale);
  if (options.now != null) out.now = options.now instanceof Date ? options.now.toISOString() : String(options.now);
  if (options.schemaUrl != null) out.schemaUrl = String(options.schemaUrl);
  if (options.fetch != null) out.fetch = options.fetch;
  return JSON.stringify(out);
}

function engineOf(analysis) {
  const e = inner.get(analysis);
  if (!e) throw new TypeError('expected a susbot Analysis');
  return e;
}

export class Analysis {
  #text;
  #report;

  /**
   * @param {string} text robots.txt content
   * @param {import('../index.d.ts').AnalysisOptions} [options]
   */
  constructor(text, options = {}) {
    this.#text = String(text);
    inner.set(this, new glue.Analysis(this.#text, optionsJson(options)));
  }

  get text() {
    return this.#text;
  }

  get report() {
    this.#report ??= JSON.parse(engineOf(this).reportJson());
    return this.#report;
  }

  get issues() {
    return this.report.issues;
  }

  get crawlers() {
    return this.report.crawlers;
  }

  get security() {
    return this.report.security;
  }

  reportJson({ pretty = false } = {}) {
    const e = engineOf(this);
    return pretty ? e.reportJsonPretty() : e.reportJson();
  }

  checkAccess(userAgent, path) {
    const tokens = Array.isArray(userAgent) ? userAgent.map(String) : [String(userAgent)];
    return JSON.parse(engineOf(this).checkAccess(JSON.stringify(tokens), String(path)));
  }

  allowed(userAgent, path) {
    return this.checkAccess(userAgent, path).allowed === true;
  }

  cleanParams(path) {
    return JSON.parse(engineOf(this).cleanParams(String(path)));
  }

  markdown() {
    return engineOf(this).markdown();
  }

  html() {
    return engineOf(this).html();
  }

  csvTabs() {
    return JSON.parse(engineOf(this).csvTabs());
  }

  tabLabels() {
    return JSON.parse(engineOf(this).tabLabels());
  }

  taggedCsv() {
    return engineOf(this).taggedCsv();
  }

  tsv(tab) {
    return engineOf(this).tsv(String(tab)) ?? null;
  }

  filename() {
    return engineOf(this).filename();
  }

  lines() {
    return JSON.parse(engineOf(this).lines());
  }

  crawlerList() {
    return JSON.parse(engineOf(this).crawlers());
  }

  /** Releases the engine memory now instead of at garbage collection. */
  free() {
    const e = inner.get(this);
    if (e) {
      inner.delete(this);
      e.free();
    }
  }

  [Symbol.dispose]() {
    this.free();
  }
}

export function diff(oldAnalysis, newAnalysis) {
  return JSON.parse(glue.diffJson(engineOf(oldAnalysis), engineOf(newAnalysis)));
}

export function diffMarkdown(oldAnalysis, newAnalysis, domain) {
  return glue.diffMarkdown(engineOf(oldAnalysis), engineOf(newAnalysis), String(domain));
}

export function defaultConfig() {
  return glue.defaultConfig();
}

export function validateConfig(toml) {
  const error = glue.validateConfig(String(toml));
  if (error) throw new Error(error);
}

export function version() {
  return glue.version();
}
