//! `susbot audit`: one file or URL, every check, any export.

use crate::net::{fetch_robots, iso_now};
use clap::{Args, ValueEnum};
use std::io::Read;
use std::path::PathBuf;
use susbot_core::model::Level;
use susbot_core::{Analysis, Options};

#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum Format {
    /// Machine-readable report (schema/report.schema.json)
    Json,
    /// Client audit in Markdown
    Markdown,
    /// The audit as a standalone HTML document
    Html,
    /// All spreadsheet tabs as one tagged CSV
    Csv,
    /// Recommended actions and issue counts, for logs
    Summary,
}

#[derive(Clone, Copy, ValueEnum, Debug, PartialEq)]
pub enum FailOn {
    Never,
    Error,
    Warning,
}

#[derive(Clone, Copy, ValueEnum, Debug, PartialEq)]
pub enum Severity {
    Never,
    High,
    Medium,
    Low,
}

#[derive(Args, Debug)]
pub struct AuditArgs {
    /// Site URL (https://example.com), a path to a robots.txt file, or "-" for stdin
    #[arg(required_unless_present = "print_default_config", default_value = "")]
    pub target: String,
    /// TOML config merged over the built-in rules (see config/default.toml)
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    /// Print the built-in configuration and exit
    #[arg(long)]
    pub print_default_config: bool,
    /// Output format
    #[arg(short, long, value_enum, default_value = "summary")]
    pub format: Format,
    /// Write the output to a file instead of stdout
    #[arg(short, long)]
    pub out: Option<PathBuf>,
    /// Language code for messages; loads <lang>.json from --locale-dir, ./locales or next to the binary
    #[arg(long, default_value = "en")]
    pub lang: String,
    /// Directory holding <lang>.json dictionaries
    #[arg(long)]
    pub locale_dir: Option<PathBuf>,
    /// Site URL assumed for a local file, enabling origin-dependent checks
    #[arg(long)]
    pub site_url: Option<String>,
    /// Exit 1 when an issue at this level or worse is present
    #[arg(long, value_enum, default_value = "never")]
    pub fail_on: FailOn,
    /// Exit 1 when a security finding at this severity or worse is present
    #[arg(long, value_enum, default_value = "never")]
    pub fail_on_security: Severity,
    /// Also refetch the file with every crawler's real User-Agent and report differences
    #[arg(long)]
    pub access_check: bool,
    /// Request timeout in seconds
    #[arg(long, default_value = "15")]
    pub timeout: u64,
}

pub fn load_locale(lang: &str, locale_dir: Option<&PathBuf>) -> Option<String> {
    if lang == "en" {
        return None;
    }
    let mut candidates = Vec::new();
    if let Some(d) = locale_dir {
        candidates.push(d.join(format!("{lang}.json")));
    }
    candidates.push(PathBuf::from("locales").join(format!("{lang}.json")));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("locales").join(format!("{lang}.json")));
        }
    }
    let found = candidates.iter().find_map(|p| std::fs::read_to_string(p).ok());
    if found.is_none() {
        eprintln!("susbot: no dictionary for \"{lang}\" found (looked in {}); messages fall back to English", candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "));
    }
    found
}

fn access_check(origin: &str, analysis: &Analysis, timeout: u64) -> String {
    let crawlers = &analysis.engine.config.crawlers;
    let robots_url = format!("{origin}/robots.txt");
    let client = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(timeout)).build();
    let get = |ua: &str| -> (u16, String) {
        match client.get(&robots_url).set("User-Agent", ua).call() {
            Ok(r) => {
                let s = r.status();
                (s, r.into_string().unwrap_or_default())
            }
            Err(ureq::Error::Status(s, r)) => (s, r.into_string().unwrap_or_default()),
            Err(e) => (0, e.to_string()),
        }
    };
    let (base_status, base_body) = get(&crawlers.browser_ua);
    let mut lines = vec![format!("{:<28} {:>6}  {}", "user-agent", "status", "body vs browser"), format!("{:<28} {:>6}  -", "browser (baseline)", base_status)];
    for c in crawlers.list.iter().filter(|c| c.ua.is_some()) {
        let (s, body) = get(c.ua.as_deref().unwrap());
        let diff = if s != base_status {
            "status differs"
        } else if body != base_body {
            "body differs"
        } else {
            "same"
        };
        lines.push(format!("{:<28} {:>6}  {diff}", c.name, s));
    }
    lines.join("\n")
}

pub fn run(cli: &AuditArgs) -> Result<u8, String> {
    if cli.print_default_config {
        print!("{}", susbot_core::config::DEFAULT_TOML);
        return Ok(0);
    }
    if cli.target.is_empty() {
        return Err("a target is required: a URL, a file, or - for stdin".into());
    }
    let config = match &cli.config {
        Some(p) => Some(std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?),
        None => None,
    };
    let locale = load_locale(&cli.lang, cli.locale_dir.as_ref());
    let is_url = cli.target.starts_with("http://") || cli.target.starts_with("https://") || (!cli.target.contains('/') && cli.target.contains('.') && !std::path::Path::new(&cli.target).exists());
    let (text, fetch, site_url, origin) = if cli.target == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).map_err(|e| e.to_string())?;
        (s, None, cli.site_url.clone(), None)
    } else if is_url {
        let raw = if cli.target.contains("://") { cli.target.clone() } else { format!("https://{}", cli.target) };
        let u = url::Url::parse(&raw).map_err(|e| format!("{raw}: {e}"))?;
        let origin = u.origin().ascii_serialization();
        let engine = susbot_core::Engine::from_toml(config.as_deref())?;
        let f = fetch_robots(&origin, &engine.config.crawlers.browser_ua, cli.timeout)?;
        let text = f.text.clone();
        let site = f.final_url.clone();
        (text, Some(f), Some(site), Some(origin))
    } else {
        let s = std::fs::read_to_string(&cli.target).map_err(|e| format!("{}: {e}", cli.target))?;
        (s, None, cli.site_url.clone(), None)
    };

    let analysis = Analysis::new(&text, Options { site_url, fetch, config, lang: Some(cli.lang.clone()), locale, now: Some(iso_now()), schema_url: Some("https://sus.bot/schema/report.schema.json".into()) })?;
    let r = &analysis.report;

    let mut output = match cli.format {
        Format::Json => analysis.report_json_pretty() + "\n",
        Format::Markdown => analysis.markdown(),
        Format::Html => analysis.html(),
        Format::Csv => analysis.tagged_csv(),
        Format::Summary => {
            let mut s = format!(
                "{}\n  errors {}, warnings {}, notes {}; security findings {}; AI training crawlers blocked {}/{}\n\n",
                r.source.final_url.clone().or(r.source.site_url.clone()).unwrap_or_else(|| cli.target.clone()),
                r.summary.issues.errors,
                r.summary.issues.warnings,
                r.summary.issues.notes,
                r.summary.security_findings,
                r.summary.ai_training.blocked,
                r.summary.ai_training.total
            );
            let mut issues: Vec<_> = r.issues.iter().filter(|i| i.level != Level::Info).collect();
            issues.sort_by(|a, b| a.level.cmp(&b.level).then(a.line.unwrap_or(u32::MAX).cmp(&b.line.unwrap_or(u32::MAX))));
            for i in issues {
                s.push_str(&format!("{:<7} {:>5}  {}\n", i.level.as_str(), i.line.map(|l| format!("L{l}")).unwrap_or_default(), i.message));
            }
            for f in &r.security {
                s.push_str(&format!("{:<7} {:>5}  Disallow: {} ({}, {})\n", f.severity, format!("L{}", f.line), f.path, f.category, f.reason));
            }
            s.push_str("\nRecommended actions:\n");
            for (i, a) in analysis.recommended_actions().iter().enumerate() {
                s.push_str(&format!("{}. {a}\n", i + 1));
            }
            s
        }
    };
    if cli.access_check {
        if let Some(origin) = &origin {
            let table = access_check(origin, &analysis, cli.timeout);
            if matches!(cli.format, Format::Summary | Format::Markdown) {
                output.push_str("\nReal access by user-agent:\n");
                output.push_str(&table);
                output.push('\n');
            } else {
                eprintln!("{table}");
            }
        } else {
            eprintln!("susbot: --access-check needs a URL target");
        }
    }
    match &cli.out {
        Some(p) => std::fs::write(p, output).map_err(|e| format!("{}: {e}", p.display()))?,
        None => print!("{output}"),
    }

    let worst_issue = r.issues.iter().map(|i| i.level).min();
    let fail_issue = match cli.fail_on {
        FailOn::Never => false,
        FailOn::Error => worst_issue == Some(Level::Error),
        FailOn::Warning => matches!(worst_issue, Some(Level::Error) | Some(Level::Warning)),
    };
    let rank = |s: &str| match s {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    };
    let worst_sec = r.security.iter().map(|f| rank(&f.severity)).min();
    let fail_sec = match cli.fail_on_security {
        Severity::Never => false,
        Severity::High => worst_sec == Some(0),
        Severity::Medium => matches!(worst_sec, Some(0) | Some(1)),
        Severity::Low => worst_sec.is_some(),
    };
    Ok(if fail_issue || fail_sec { 1 } else { 0 })
}
