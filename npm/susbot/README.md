# susbot

The npm package of [sus.bot](https://sus.bot/), a robots.txt checker. It tells you which search engines and AI crawlers a site lets in, what is wrong with the file, and which paths it gives away.

It contains two things, both built from the Rust engine behind sus.bot:

- the `susbot` command, a native binary for Linux, macOS and Windows;
- a JavaScript API over the same engine compiled to WebAssembly, for Node and browsers.

```
npm install susbot          # the API, plus the command in node_modules/.bin
npx susbot https://example.com
```

## Command line

```
npx susbot https://example.com                     # audit a live site
npx susbot robots.txt --format markdown --out audit.md
npx susbot diff old.txt new.txt                    # what changed, crawler by crawler
npx susbot --help
```

It is the same program as `cargo install susbot` and `pip install susbot`. Exit codes: 0 on success, 1 when a `--fail-on` or `--fail-on-security` threshold is met, 2 on errors. npm installs the binary for your platform as an optional dependency (`susbot-linux-x64`, `susbot-darwin-arm64` and so on); installs with `--omit=optional` get the API only.

## JavaScript

```js
import { Analysis, diff } from 'susbot';

const text = await (await fetch('https://example.com/robots.txt')).text();
const a = new Analysis(text, { siteUrl: 'https://example.com/' });

a.allowed('GPTBot', '/blog/post');       // false
a.checkAccess('Googlebot', '/admin/');   // the verdict and the rule that decided it
a.report.summary;                        // counts, default policy, AI training, platform
a.issues;                                // lint findings with stable ids
a.security;                              // disallowed paths that look sensitive
a.markdown();                            // the client audit; also html(), csvTabs(), tsv()

diff(new Analysis(oldText), a);          // crawler verdict flips, new sensitive paths, issues
```

In Node the engine loads on import. In browsers and bundlers, call `await init()` once first; it fetches the `.wasm` file next to the module. Types are included. Options: `config` for a TOML file merged over `defaultConfig()`, `locale` with `lang` for a translated report, `now`, `schemaUrl`, `fetch`. The report follows the [published JSON schema](https://sus.bot/schema/report.schema.json).

## Links

- Website: [sus.bot](https://sus.bot/)
- Source, GitHub Action and issues: [github.com/sitefig/robots](https://github.com/sitefig/robots)

## Licence

[PolyForm Noncommercial 1.0.0](LICENSE.md): free for noncommercial use. Commercial use needs a licence from [Sitefig](https://sitefig.eu/).
