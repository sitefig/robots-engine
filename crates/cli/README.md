# susbot

The command-line version of [sus.bot](https://sus.bot/), a robots.txt checker. It reads a site's robots.txt and reports which search engines and AI crawlers may visit which pages, what is wrong with the file, and which paths it reveals to anyone who reads it. The same engine runs the website, so results match.

## Install

```
cargo install susbot
```

This installs a program called `susbot`. `cargo install susbot-cli` installs the same program.

## Use

```
susbot https://example.com                         # summary with issues, security findings and recommended actions
susbot https://example.com --format markdown --out audit.md
susbot https://example.com --format json           # the full report, see the schema in the repository
susbot robots.txt --site-url https://example.com   # a local file
susbot https://example.com --fail-on warning       # exit 1 in CI when there are warnings or errors
susbot https://example.com --fail-on-security high # exit 1 when a high-risk path is exposed
susbot https://example.com --access-check          # refetch with every crawler's user-agent
susbot --print-default-config > my-rules.toml      # every rule, to adjust and pass with --config
```

Other commands:

```
susbot diff old.txt new.txt        # what changed for crawlers, new sensitive paths, issues, sitemaps
susbot track --config sites.json --data-dir data   # keep snapshots of many sites and report changes
susbot crawl --input domains.csv --out audit.jsonl.gz   # audit a list of domains into a dataset
```

Exit codes: 0 when all is well, 1 when findings reach `--fail-on` or `--fail-on-security`, 2 on fetch or configuration errors.

Reports are in English by default. For another EU language, pass `--lang` with `--locale-dir` pointing at the `locales` folder of the [repository](https://github.com/sitefig/robots).

## More

- Website: [sus.bot](https://sus.bot/)
- GitHub Action and source: [github.com/sitefig/robots](https://github.com/sitefig/robots)
- The engine on its own: [susbot-core](https://crates.io/crates/susbot-core)

## Licence

[PolyForm Noncommercial 1.0.0](LICENSE.md): free for noncommercial use. Commercial use, including running it in a company's CI, needs a licence from [Sitefig](https://sitefig.eu/).
