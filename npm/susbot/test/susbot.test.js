// Tests for the npm package: node --test npm/susbot/test/
// Needs npm/susbot/wasm/ (scripts/build-npm-wasm.sh). The CLI tests need a
// susbot binary in SUSBOT_BINARY and are skipped without one.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import * as susbot from '../node.js';
import * as web from '../web.js';

const KITCHEN_SINK = readFileSync(new URL('../../../examples/kitchen-sink.robots.txt', import.meta.url), 'utf8');
const GPTBOT_BLOCKED = 'User-agent: GPTBot\nDisallow: /\n\nUser-agent: *\nDisallow: /admin/\n';
const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));

test('version matches the package', () => {
  assert.equal(susbot.version(), pkg.version);
});

test('access', () => {
  const a = new susbot.Analysis(GPTBOT_BLOCKED, { siteUrl: 'https://example.com/' });
  assert.equal(a.allowed('GPTBot', '/page'), false);
  assert.equal(a.allowed('Googlebot', '/page'), true);
  assert.equal(a.allowed(['Googlebot'], '/admin/x'), false);
  const access = a.checkAccess('GPTBot', '/robots.txt');
  assert.ok(access.allowed && access.always);
  assert.equal(a.text, GPTBOT_BLOCKED);
  a.free();
  assert.throws(() => a.report, TypeError);
});

test('report and exports', () => {
  const a = new susbot.Analysis(KITCHEN_SINK, { siteUrl: 'https://www.example.com/', now: new Date('2026-01-01T00:00:00Z') });
  assert.equal(a.report.summary.platform, 'WordPress');
  assert.equal(a.report.generatedAt.slice(0, 10), '2026-01-01');
  assert.ok(a.issues.some((i) => i.id === 'parser.ruleBeforeAgent'));
  assert.ok(a.security.some((f) => f.severity === 'high'));
  assert.ok(a.crawlers.some((c) => c.name === 'GPTBot'));
  assert.deepEqual(JSON.parse(a.reportJson({ pretty: true })).summary, a.report.summary);
  assert.match(a.markdown(), /^#/);
  assert.match(a.html(), /<html/);
  const tabs = a.csvTabs();
  assert.deepEqual(Object.keys(tabs).sort(), Object.keys(a.tabLabels()).sort());
  assert.equal(typeof a.tsv(Object.keys(tabs)[0]), 'string');
  assert.equal(a.tsv('no-such-tab'), null);
  assert.ok(a.taggedCsv().length > 0);
  assert.ok(a.filename());
  assert.ok(a.lines().length > 0);
});

test('config', () => {
  assert.match(susbot.defaultConfig(), /\[crawlers\]/);
  susbot.validateConfig('[rules]\ndisabled = ["seo.trailingSlash"]\n');
  assert.throws(() => susbot.validateConfig('[rules\n'));
  assert.throws(() => new susbot.Analysis('User-agent: *', { config: 'not = [valid' }));
});

test('diff', () => {
  const old = new susbot.Analysis('User-agent: *\nAllow: /\n', { siteUrl: 'https://example.com/' });
  const now = new susbot.Analysis(GPTBOT_BLOCKED, { siteUrl: 'https://example.com/' });
  const d = susbot.diff(old, now);
  assert.equal(d.is_changed, true);
  assert.ok(d.bot_changes.some((b) => b.name === 'GPTBot' && b.new_verdict === 'blocked'));
  assert.match(susbot.diffMarkdown(old, now, 'example.com'), /example\.com/);
  assert.equal(susbot.diff(old, old).is_changed, false);
});

test('the web entry point shares the engine once initialised', async () => {
  await web.init(readFileSync(new URL('../wasm/susbot_wasm_bg.wasm', import.meta.url)));
  assert.equal(web.version(), pkg.version);
  assert.equal(new web.Analysis(GPTBOT_BLOCKED).allowed('GPTBot', '/'), false);
});

const bin = fileURLToPath(new URL('../bin/susbot.js', import.meta.url));
const cli = (args) => spawnSync(process.execPath, [bin, ...args], { encoding: 'utf8' });

test('cli', { skip: !process.env.SUSBOT_BINARY && 'SUSBOT_BINARY is not set' }, () => {
  const v = cli(['--version']);
  assert.equal(v.status, 0, v.stderr);
  assert.equal(v.stdout.trim(), `susbot ${pkg.version}`);
  const json = cli([fileURLToPath(new URL('../../../examples/kitchen-sink.robots.txt', import.meta.url)), '--format', 'json']);
  assert.equal(json.status, 0, json.stderr);
  assert.equal(JSON.parse(json.stdout).summary.platform, 'WordPress');
  assert.equal(cli(['--bogus']).status, 2);
});

test('cli without a binary explains itself', () => {
  const env = { ...process.env };
  delete env.SUSBOT_BINARY;
  const r = spawnSync(process.execPath, [bin, '--version'], { encoding: 'utf8', env });
  // Either the platform package is installed (a packed install) or the message names it.
  if (r.status !== 0) {
    assert.equal(r.status, 2);
    assert.match(r.stderr, /susbot-[a-z0-9]+-[a-z0-9]+/);
  }
});
