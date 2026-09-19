// Types for the susbot package. The report follows the published JSON schema:
// https://sus.bot/schema/report.schema.json

export type Level = 'error' | 'warning' | 'note';
export type Verdict = 'open' | 'partial' | 'blocked';

export interface AnalysisOptions {
  /** Origin the file was served from; enables the checks that depend on it. */
  siteUrl?: string;
  /** TOML merged over the default configuration (arrays replace). */
  config?: string;
  /** Language code of `locale`, recorded in the report. */
  lang?: string;
  /** A locale dictionary (locales/<code>.json in the repository), as an object or JSON text. */
  locale?: Record<string, unknown> | string;
  /** Timestamp for generatedAt and date notes. */
  now?: string | Date;
  /** Absolute URL of the report schema, for $schema. */
  schemaUrl?: string;
  /** How the file was fetched (FetchInfo in the schema), for redirect and cross-host findings. */
  fetch?: Record<string, unknown>;
}

export interface Issue {
  level: Level;
  kind: string;
  /** Stable across languages. */
  id: string;
  line: number | null;
  message: string;
}

export interface Crawler {
  name: string;
  category: string;
  categoryLabel: string;
  tokens: string[];
  note: string | null;
  groupUsed: string | null;
  ownGroup: boolean;
  verdict: Verdict;
  rootAllowed: boolean;
  allowRules: number;
  disallowRules: number;
  crawlDelay: number | null;
}

export interface SecurityFinding {
  path: string;
  line: number;
  userAgents: string[];
  category: string;
  severity: 'high' | 'medium' | 'low';
  reason: string;
}

export interface Summary {
  groups: number;
  rules: number;
  sitemaps: number;
  issues: { errors: number; warnings: number; notes: number };
  securityFindings: number;
  reconFindings: number;
  defaultPolicy: { hasStarGroup: boolean; verdict: Verdict; allowRules: number; disallowRules: number };
  aiTraining: { blocked: number; total: number };
  platform: string | null;
}

export interface Report {
  $schema: string;
  schemaVersion: string;
  generatedAt: string;
  language: string;
  summary: Summary;
  issues: Issue[];
  crawlers: Crawler[];
  security: SecurityFinding[];
  [key: string]: unknown;
}

export interface Rule {
  directive: string;
  value: string;
  line: number;
  [key: string]: unknown;
}

export interface Access {
  /** The user-agent token whose group applied, or null for the * group. */
  token: string | null;
  specific: boolean;
  crawlDelay: number | null;
  /** /robots.txt itself, always allowed. */
  always: boolean;
  allowed: boolean;
  /** The rule that decided, or null when none matched. */
  rule: Rule | null;
  path: string;
}

export interface Cleaned {
  path: string;
  removed: string[];
}

export interface Diff {
  is_changed: boolean;
  text_changed: boolean;
  bot_changes: { name: string; category: string; old_verdict: Verdict; new_verdict: Verdict }[];
  new_security_findings: { path: string; category: string; severity: string; reason: string }[];
  resolved_security_findings: { path: string; category: string; severity: string; reason: string }[];
  new_issues: { id: string; level: Level; message: string }[];
  resolved_issues: { id: string; level: Level; message: string }[];
  added_sitemaps: string[];
  removed_sitemaps: string[];
  old_rules_count: number;
  new_rules_count: number;
  unified_text_diff: string;
}

/** One robots.txt file, parsed, matched, linted and summarised. Throws on a bad config or locale. */
export class Analysis {
  constructor(text: string, options?: AnalysisOptions);
  readonly text: string;
  /** The full report, parsed once. */
  readonly report: Report;
  readonly issues: Issue[];
  readonly crawlers: Crawler[];
  readonly security: SecurityFinding[];
  reportJson(options?: { pretty?: boolean }): string;
  /** `userAgent` is one product token or several, most specific first. */
  checkAccess(userAgent: string | string[], path: string): Access;
  allowed(userAgent: string | string[], path: string): boolean;
  cleanParams(path: string): Cleaned;
  /** The client audit as Markdown. */
  markdown(): string;
  /** The client audit as a standalone HTML document. */
  html(): string;
  /** CSV text per tab id. */
  csvTabs(): Record<string, string>;
  /** Translated label per tab id. */
  tabLabels(): Record<string, string>;
  taggedCsv(): string;
  tsv(tab: string): string | null;
  filename(): string;
  /** Per-line kinds of the parsed file. */
  lines(): { n: number; kind: string }[];
  /** The crawler list of the active configuration. */
  crawlerList(): Record<string, unknown>;
  /** Releases the engine memory now instead of at garbage collection. */
  free(): void;
}

/** Loads the engine. Required once in browsers; a no-op in Node. */
export function init(input?: string | URL | Request | Response | BufferSource | WebAssembly.Module): Promise<void>;
/** Semantic diff of two analyses built with the same options. */
export function diff(oldAnalysis: Analysis, newAnalysis: Analysis): Diff;
export function diffMarkdown(oldAnalysis: Analysis, newAnalysis: Analysis, domain: string): string;
/** The embedded default configuration, TOML text. */
export function defaultConfig(): string;
/** Throws when the TOML does not merge over the defaults. */
export function validateConfig(toml: string): void;
export function version(): string;
