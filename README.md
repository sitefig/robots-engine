# sus.bot engine

The engine behind [sus.bot](https://sus.bot/) and the `susbot` command: it reads a robots.txt and reports what it means for search engines and AI crawlers, what is wrong with it, and what it gives away. One Rust crate does the analysis; the CLI, the GitHub Action, the Python package, the npm package and the browser build are all ways to run it, so a check behaves the same everywhere.

The website that runs this in the browser lives in [sitefig/robots](https://github.com/sitefig/robots) and carries this repository as a submodule.

## Install

```
cargo install susbot                  # crates.io; susbot-cli is the same crate under its long name
pip install susbot                    # PyPI; adds Python bindings
npm install -g @sitefig/susbot        # npm; adds the engine as WebAssembly for Node and browsers
```

All three install the same `susbot` binary. `cargo install` builds it; the other two ship prebuilt binaries for Linux, macOS and Windows.

## susbot audit

```
susbot audit <target> [options]
susbot <target> [options]             # audit is the default, so this is the same
```

`<target>` is a site URL (`https://example.com`, the path is ignored and `/robots.txt` is fetched), a path to a local file, or `-` for stdin.

| Option | Meaning |
| --- | --- |
| `-f, --format <fmt>` | `summary` (default), `json`, `markdown`, `html`, `csv` |
| `-o, --out <file>` | write to a file instead of stdout |
| `-c, --config <file>` | TOML merged over the built-in rules |
| `--print-default-config` | print the built-in configuration and exit |
| `--site-url <url>` | the origin to assume for a local file, which enables the checks that depend on it |
| `--access-check` | refetch the file as each crawler, with its real user agent, and report servers that answer differently |
| `--fail-on <level>` | exit 1 at `error` or `warning` and worse; `never` by default |
| `--fail-on-security <severity>` | exit 1 at `high`, `medium` or `low` and worse; `never` by default |
| `--lang <code>` | language for the findings; loads `<code>.json` from `--locale-dir`, `./locales` or next to the binary |
| `--locale-dir <dir>` | where those dictionaries are |
| `--timeout <seconds>` | request timeout, 15 by default |

```
susbot https://example.com                                    # counts, findings and recommended actions
susbot https://example.com --format json | jq .summary        # the full report, schema below
susbot https://example.com --format markdown --out audit.md   # a client-ready audit
susbot robots.txt --site-url https://example.com              # a local file, checked as if served there
curl -s https://example.com/robots.txt | susbot - --site-url https://example.com
susbot https://example.com --lang de --locale-dir locales     # findings in German
susbot https://example.com --fail-on warning --fail-on-security high
```

Exit codes: **0** clean, **1** a threshold was reached, **2** a fetch or configuration error. That makes `--fail-on` usable as a gate in any CI system.

The JSON report is the same object every other format is built from, and it validates against [`schema/report.schema.json`](schema/report.schema.json). `schemaVersion` follows semantic versioning, so a minor bump only adds fields.

## susbot diff

```
susbot diff <old-file> <new-file> [options]
```

A semantic diff, not a text one: crawler verdicts that flipped, security findings that appeared or went away, issues introduced or fixed, sitemaps added or removed, and a unified text diff at the end.

| Option | Meaning |
| --- | --- |
| `-d, --domain <domain>` | the domain the two files belong to, used in the wording |
| `-f, --format <fmt>` | `markdown` (default), `json`, `social` for a short post |
| `--diff-url <url>` | link included in the social form |
| `--site-url <url>` | origin assumed for both files |
| `-c, --config <file>` | TOML merged over the built-in rules |
| `--fail-on-change` | exit 1 when the directives differ |

```
susbot diff yesterday.txt today.txt --domain example.com
susbot diff yesterday.txt today.txt --fail-on-change    # a gate: fail the job when the file moves
```

Both sides are analysed with the same options, so only the text decides whether something changed. Comment-only edits do not count as a change; the raw text diff still shows them.

## susbot track

Fetch a list of domains, diff each against its stored snapshot, write what changed, and alert.

```
susbot track --config domains.json --data-dir data/tracked [options]
```

| Option | Meaning |
| --- | --- |
| `-c, --config <file>` | the list, JSON, default `config/famous-100.json` |
| `-d, --data-dir <dir>` | snapshot directory, one per domain, default `data/famous-100` |
| `--summary-out <file>` | a Markdown digest of the run, for a job summary |
| `--social-out <file>` | social drafts for the sites that changed |
| `--changed-out <file>` | the changed domains, one per line, for a commit message or a gate |
| `--webhook <url>` | Slack or Discord incoming webhook; a site's own `webhook_url` wins |
| `--webhook-high-impact-only` | only crawler flips, new high-severity paths and new errors |
| `--git-base-url <url>` | prefix for links to the stored snapshots |
| `--title <text>` | heading of the generated leaderboard |
| `--rules <file>` | TOML merged over the built-in rules |
| `--concurrency <n>` | parallel fetches, 16 by default |
| `--timeout <seconds>` | request timeout, 10 by default |
| `--dry-run` | fetch and diff, write nothing |

The list is JSON:

```json
[
  { "domain": "example.com", "name": "Example", "category": "retail", "enabled": true },
  { "domain": "shop.example", "webhook_url": "https://hooks.slack.com/services/..." }
]
```

Each domain gets a directory holding `robots.txt` and `meta.json`, so the git history of that directory is the change log. A 404 or 410 is stored as an empty file, because that is what a crawler sees; a 403, 429 or 5xx, or an HTML body, keeps the last snapshot rather than recording a wall as a change. The leaderboard `README.md` in the data directory is rewritten only when something moved.

## susbot crawl

Bulk audit into a dataset.

```
susbot crawl --input domains.csv --out dataset.jsonl.gz [options]
```

| Option | Meaning |
| --- | --- |
| `-i, --input <file>` | one domain per line, or a CSV whose last column is the domain (Tranco format) |
| `-o, --out <file>` | gzip JSON Lines, one record per domain |
| `--summary <file>` | JSON counts per crawler verdict, platform and status |
| `--limit <n>` / `--offset <n>` | take a slice, or resume where a run stopped |
| `--concurrency <n>` | parallel fetches, 32 by default |
| `--timeout <seconds>` | request timeout, 10 by default |
| `--rules <file>` | TOML merged over the built-in rules |

One engine is shared across the thread pool, so a million domains cost one configuration parse. The output is JSON Lines that DuckDB or pandas read directly:

```sql
SELECT summary->>'platform' AS platform, count(*)
FROM read_json_auto('dataset.jsonl.gz') GROUP BY 1 ORDER BY 2 DESC;
```

## GitHub Action

```yaml
- uses: sitefig/robots-engine@main
  with:
    url: https://example.com
    config: .github/robots-audit.toml   # optional
    format: markdown                    # also written to the job summary
    fail-on: warning
    fail-on-security: high
    lang: en
    access-check: 'false'
```

It builds the CLI from this repository with a cached Rust toolchain and prints the report to the log. A non-zero exit fails the job, so the thresholds above are the gate.

## Configuration

`config/default.toml` holds every rule: which checks run, the crawler list, security signatures and severities, platform fingerprints, generator fingerprints, cloud providers, API and data-feed patterns, file-extension risks and comment detectors. A user file is merged over it, where tables and scalars override and arrays replace.

```toml
[checks]
security = false               # skip the sensitive-path scan

[rules]
disabled = ["seo.caseSensitive", "parser.noSitemap"]
[rules.levels]
"seo.trailingSlash" = "info"   # error, warning or info

[security]
ignore = ['^/uploads/']        # never report these Disallow paths

[[security.signatures]]        # arrays replace: copy the default list to extend it
category = "admin"
severity = "high"
reason = "Our back office"
keywords = ["backoffice-v2"]
```

`susbot --print-default-config > my-rules.toml` gives you the whole file to start from. Regexes use [fancy-regex](https://docs.rs/fancy-regex/) syntax, so `(?i)` and lookaround work.

## As a library

```rust
use susbot_core::{Analysis, Options};

let report = Analysis::new(text, Options { site_url: Some("https://example.com".into()), ..Default::default() })?
    .report_json_pretty();
```

```python
import susbot
a = susbot.Analysis(text, site_url="https://example.com/")
a.allowed("GPTBot", "/blog/post")   # False
a.report["summary"]                 # counts, default policy, AI training, platform
```

```js
import { Analysis, diff } from '@sitefig/susbot';   // await init() first in a browser
new Analysis(text, { siteUrl: 'https://example.com/' }).allowed('GPTBot', '/blog/post');
```

## What it checks

Rule matching follows [RFC 9309](https://www.rfc-editor.org/rfc/rfc9309) as implemented by Google's open-source matcher: tokens most specific first, `*` and `$` wildcards, longest match wins, Allow wins a tie, `/robots.txt` always allowed.

On top of that: lint findings (rules before any User-agent, misspellings found by edit distance, directives that are not RFC 9309 named for what they are, and content that does not belong in the file at all, from HTML markup and caching-plugin output to stack traces, injected spam and UTF-16 text), SEO traps, sitemap hygiene, fetch-level findings for how the file was served, sensitive Disallow paths, and reconnaissance covering the platform, the tool that wrote the file, cloud buckets, other hostnames, API endpoints, data feeds, file types and comment metadata.

Levels come from measurement, not taste: a scan of the robots.txt of 36 million hosts decides what is a warning and what is a note, so a mistake that appears on a quarter of the web does not shout.

## Layout

```
crates/core       the engine: parser, matcher, checks, security, recon, exports
crates/cli        the susbot command (audit, diff, track, crawl)
crates/wasm       browser bindings, built to WebAssembly
crates/python     the PyPI package (PyO3, maturin)
crates/susbot     a thin crate so `cargo install susbot` works
npm/susbot        the npm package: the WebAssembly engine plus the command
config/           default.toml: every check, crawler and signature
po/, locales/     translations; the JSON is generated from the .po files
schema/           the JSON report schema
examples/         kitchen-sink.robots.txt triggers every check
data/             what the tracking workflows commit
action.yml        the composite GitHub Action
```

## Working on it

```
cargo test --workspace                 # engine and CLI tests
npm test                               # translations: po round trip, locales, referenced keys
npm run cli -- https://example.com     # the CLI through cargo
npm run i18n:sync                      # after editing po/en.po
./scripts/build-wasm.sh                # WebAssembly, for the website
```

Toolchain: stable Rust with the `wasm32-unknown-unknown` target, `wasm-bindgen-cli` at the version pinned in `crates/wasm/Cargo.toml`, and Node 24. Releases go out on a version tag; CLAUDE.md holds the order and what publishes where.

## Licence

Source-available under the [PolyForm Noncommercial License 1.0.0](LICENSE.md). Noncommercial use is free, including personal use, research, education, charities and public bodies. Commercial use, including running the CLI or the Action in a company's CI, needs a licence from [Sitefig](https://sitefig.eu).
