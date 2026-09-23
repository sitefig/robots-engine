# susbot

The Python package of [sus.bot](https://sus.bot/), a robots.txt checker. It tells you which search engines and AI crawlers a site lets in, what is wrong with the file, and which paths it gives away. It ships the Rust engine behind sus.bot as Python bindings and the `susbot` command.

```
pip install susbot
```

Wheels are built for Linux, macOS and Windows on CPython 3.10 and later.

## Command line

```
susbot https://example.com                       # audit a live site
susbot robots.txt --format markdown --out audit.md
susbot diff old.txt new.txt                      # what changed, crawler by crawler
python -m susbot --help
```

It is the same program as `cargo install susbot`. Exit codes: 0 on success, 1 when a `--fail-on` or `--fail-on-security` threshold is met, 2 on errors.

## Python

```python
import susbot

text = open("robots.txt", encoding="utf-8").read()
a = susbot.Analysis(text, site_url="https://example.com/")

a.allowed("GPTBot", "/blog/post")        # False
a.check_access("Googlebot", "/admin/")   # the verdict and the rule that decided it
a.report["summary"]                      # counts, default policy, AI training, platform
a.issues                                 # lint findings with stable ids
a.security                               # disallowed paths that look sensitive
a.markdown()                             # the client audit; also html(), csv_tabs(), tsv()

old = susbot.Analysis(old_text, site_url="https://example.com/")
susbot.diff(old, a)                      # crawler verdict flips, new sensitive paths, issues
```

`Analysis` takes `config=` for a TOML file merged over `susbot.default_config()`, and `locale=` with `lang=` for a translated report. The report follows the [published JSON schema](https://sus.bot/schema/report.schema.json). The package never fetches anything itself; the `susbot` command does.

## Links

- Website: [sus.bot](https://sus.bot/)
- The same tool elsewhere: [susbot on crates.io](https://crates.io/crates/susbot), [@sitefig/susbot on npm](https://www.npmjs.com/package/@sitefig/susbot)
- Source, GitHub Action and issues: [github.com/sitefig/robots-engine](https://github.com/sitefig/robots-engine)

## Licence

[PolyForm Noncommercial 1.0.0](LICENSE.md): free for noncommercial use. Commercial use needs a licence from [Sitefig](https://sitefig.eu/).
