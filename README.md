# sus.bot engine

The engine behind [sus.bot](https://sus.bot/): it reads a robots.txt and says what it means for search engines and AI crawlers, what is wrong with it, and what it gives away. One Rust crate does the analysis; everything here is a way to run it.

| Where | Package | Install |
| --- | --- | --- |
| Command line | [susbot](https://crates.io/crates/susbot) | `cargo install susbot` |
| Python | [susbot](https://pypi.org/project/susbot/) | `pip install susbot` |
| Node and browsers | [@sitefig/susbot](https://www.npmjs.com/package/@sitefig/susbot) | `npm install @sitefig/susbot` |
| GitHub Action | this repository | `uses: sitefig/robots-engine@main` |

The website that runs this engine in the browser lives in [sitefig/robots](https://github.com/sitefig/robots), which carries this repository as a submodule.

## What it reports

- rule matching as Google's RFC 9309 implementation does it, per crawler, with the rule that decided,
- lint findings: rules before any User-agent, misspelt directives found by edit distance, unsupported directives named for what they are, and what does not belong in the file at all, such as HTML markup, plugin output, stack traces or injected spam,
- SEO traps: the trailing-slash trap, self-blocks, and what the `*` group blocks for everyone,
- security notes: Disallow rules that advertise admin panels, staging sites, backups, config files, private APIs, user data, version files and installers,
- reconnaissance: the platform, the tool that wrote the file, cloud buckets, other hostnames, API endpoints, data feeds, file types, and the metadata left in comments,
- exports: a client audit in Markdown or HTML, four spreadsheet tabs as CSV or TSV, and a JSON report that validates against `schema/report.schema.json`.

Findings and their levels come from a scan of the robots.txt of 36 million hosts, so a rule that fires on a quarter of the web is a note, not a warning.

## Layout

```
crates/core       the engine: parser, matcher, checks, security, recon, exports
crates/cli        the susbot command (audit, diff, track, crawl)
crates/wasm       browser bindings, built to WebAssembly
crates/python     the PyPI package (PyO3, maturin)
crates/susbot     a thin crate so `cargo install susbot` works
npm/susbot        the npm package: the WebAssembly engine plus the command
config/           default.toml: every check, crawler and signature
locales/          generated dictionaries; po/ holds the translations
schema/           the JSON report schema
examples/         kitchen-sink.robots.txt triggers every check
data/             what the tracking workflows commit
action.yml        the composite GitHub Action
```

## Working on it

```
cargo test --workspace     # engine and CLI tests
npm test                   # translations: po round trip, locales, referenced keys
npm run cli -- <url|file>   # run the CLI
npm run i18n:sync          # after editing po/en.po
./scripts/build-wasm.sh    # WebAssembly for the website
```

Releases go out on a version tag: see CLAUDE.md for the order and what publishes where.

## Licence

Source-available under the [PolyForm Noncommercial License 1.0.0](LICENSE.md). Noncommercial use is free, including personal use, research, education, charities and public bodies. Commercial use, including running the CLI or the Action in a company's CI, needs a licence from [Sitefig](https://sitefig.eu).
