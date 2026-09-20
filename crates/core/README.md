# susbot-core

The robots.txt analysis engine behind [sus.bot](https://sus.bot/) and the [`susbot`](https://crates.io/crates/susbot-cli) command-line tool. It parses a robots.txt, matches paths the way Google's RFC 9309 implementation does, and reports:

- what the file means for each known search engine and AI crawler,
- lint findings, SEO traps and sitemap problems,
- Disallow rules that point at sensitive places (admin panels, backups, staging, private APIs),
- what the file reveals: the platform, cloud buckets, other hostnames, API endpoints, data feeds, file types and comment metadata,
- exports: a JSON report, a Markdown or HTML audit, and CSV tabs.

It does no network I/O; the caller fetches the file. Every rule comes from a TOML configuration (the default is embedded) that a user file can extend.

```rust
use susbot_core::{Analysis, Options};

let analysis = Analysis::new("User-agent: GPTBot\nDisallow: /\n", Options::default())?;
println!("{}", analysis.report_json_pretty());
# Ok::<(), String>(())
```

Source, schema and configuration reference: [github.com/sitefig/robots](https://github.com/sitefig/robots).

- The same tool elsewhere: [susbot on PyPI](https://pypi.org/project/susbot/), [@sitefig/susbot on npm](https://www.npmjs.com/package/@sitefig/susbot)

## Licence

[PolyForm Noncommercial 1.0.0](LICENSE.md): free for noncommercial use. Commercial use needs a licence from [Sitefig](https://sitefig.eu/).
